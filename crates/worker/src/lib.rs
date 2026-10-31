//! The nolabs Cloudflare Worker: the HTTP API, the `nolabs-render` queue consumer and the 30-minute cron, all thin
//! adapters over `nolabs-server`, plus the embedded web app and Supabase's auth email hook. CORS is limited to the
//! native apps' origins (the web app is same-origin).

mod adapters;
mod assets;
mod respond;
mod router;
mod streams;

use nolabs_server::ports::JobMessage;
use worker::{Context, Env, MessageBatch, Request, Response, ScheduleContext, ScheduledEvent, event};

#[event(fetch)]
pub async fn fetch(req: Request, env: Env, _ctx: Context) -> worker::Result<Response> {
    let path = req.path();
    let csp = respond::csp(env.var("SUPABASE_URL").ok().map(|v| v.to_string()).as_deref());
    let resp = if path == "/api" || path.starts_with("/api/") {
        router::api(req, &env).await
    } else if path == "/hooks/auth-email" {
        router::auth_email_hook(req, &env).await
    } else {
        match req.method() {
            worker::Method::Get | worker::Method::Head => {
                let etag = req.headers().get("if-none-match").ok().flatten();
                assets::serve(&path, req.method() == worker::Method::Head, etag.as_deref())
            }
            _ => router::method_not_allowed(),
        }
    };
    Ok(respond::secure(resp, &csp))
}

/// Consumer settings expected on the queue: `max_batch_size = 1`, `max_retries = 0` (retries are decided by
/// `nolabs_core::job` and re-sent with a delay). Every message is acknowledged; lost work is recovered by the cron.
#[event(queue)]
pub async fn queue(batch: MessageBatch<serde_json::Value>, env: Env, _ctx: Context) -> worker::Result<()> {
    let app = match adapters::app(&env) {
        Ok(app) => app,
        Err(e) => {
            worker::console_error!("queue: {}", e);
            batch.ack_all();
            return Ok(());
        }
    };
    for raw in batch.raw_iter() {
        match serde_wasm_bindgen::from_value::<JobMessage>(raw.body()) {
            Ok(message) => match app.handle_job(&message).await {
                Ok(outcome) => worker::console_log!("job {}: {:?}", message.job_id, outcome),
                Err(e) => worker::console_error!("job {}: {}", message.job_id, e),
            },
            Err(e) => worker::console_error!("queue: malformed message: {}", e),
        }
    }
    batch.ack_all();
    Ok(())
}

#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    match adapters::app(&env) {
        Ok(app) => {
            let report = app.run_cron().await;
            worker::console_log!("cron: {:?}", report);
        }
        Err(e) => worker::console_error!("cron: {}", e),
    }
}
