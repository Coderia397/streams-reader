//! The `/api` dispatcher: route table and guards from `nolabs_server::routes`, then one handler per route. JSON
//! routes call the services; body, audio and recording routes stream between the client and R2. Every route is
//! authenticated with the caller's Supabase access token (audio also with an audio-URL token); CORS opens the API to
//! the native apps' origins (see `nolabs_server::auth::guard`).

use nolabs_core::proto::ApiError;
use nolabs_core::proto::api::{API_VERSION_HEADER, AUDIO_TOKEN_PARAM, CreateVoice, RenderRequest};
use nolabs_server::App;
use nolabs_server::auth::AuthUser;
use nolabs_server::auth::guard::{check_api_version, cors_allows, cors_headers};
use nolabs_server::error::{parse_json, too_large};
use nolabs_server::routes::Route;
use nolabs_server::services::audio::{AUDIO_CACHE_CONTROL, AUDIO_CONTENT_TYPE};
use nolabs_server::services::auth_email::{HookError, HookHeaders};
use nolabs_server::services::render::parse_status_keys;
use worker::{Env, Request, Response};

use crate::adapters;
use crate::respond::{self, internal};
use crate::streams;

/// JSON request bodies are small (a render request is at most 64 items of 1000 chars).
const MAX_JSON_BYTES: u64 = 1024 * 1024;
/// A Send Email hook payload is a user object and a few tokens.
const MAX_HOOK_BYTES: u64 = 64 * 1024;

fn header(req: &Request, name: &str) -> Option<String> {
    req.headers().get(name).ok().flatten()
}

fn content_length(req: &Request) -> Option<u64> {
    header(req, "content-length").and_then(|v| v.trim().parse().ok())
}

async fn bytes_limited(req: &mut Request, limit: u64) -> Result<Vec<u8>, ApiError> {
    if content_length(req).is_some_and(|n| n > limit) {
        return Err(too_large("request body too large"));
    }
    let bytes = req.bytes().await.map_err(internal)?;
    if bytes.len() as u64 > limit {
        return Err(too_large("request body too large"));
    }
    Ok(bytes)
}

async fn body<T: serde::de::DeserializeOwned>(req: &mut Request) -> Result<T, ApiError> {
    parse_json(&bytes_limited(req, MAX_JSON_BYTES).await?)
}

fn query(req: &Request, name: &str) -> Option<String> {
    req.url().ok()?.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned())
}

/// The request's Origin when it may read our responses (CORS), else None (same-origin and non-browser callers send
/// no Origin or one we simply do not echo).
fn allowed_origin(req: &Request, env: &Env) -> Option<String> {
    let origin = header(req, "origin")?;
    let site = env.var("SITE_URL").map(|v| v.to_string()).unwrap_or_default();
    let own = req.url().ok().map(|u| u.origin().ascii_serialization()).unwrap_or_default();
    (cors_allows(&origin, &site) || origin == own).then_some(origin)
}

fn with_cors(mut resp: Response, origin: Option<&str>) -> Response {
    if let Some(o) = origin {
        let h = resp.headers_mut();
        for (k, v) in cors_headers(o) {
            let _ = h.set(k, &v);
        }
    }
    resp
}

/// Handle one API request; every failure becomes the JSON error body.
pub async fn api(req: Request, env: &Env) -> Response {
    let origin = allowed_origin(&req, env);
    if req.method() == worker::Method::Options {
        // Preflight: answer for allowed origins only; others get no CORS headers and the browser refuses.
        return with_cors(respond::no_content(), origin.as_deref());
    }
    let resp = match dispatch(req, env).await {
        Ok(resp) => resp,
        Err(e) => {
            if e.status() >= 500 {
                worker::console_error!("api error: {}", e);
            }
            respond::error(&e)
        }
    };
    with_cors(resp, origin.as_deref())
}

async fn authenticate(app: &App, req: &Request, route: &Route) -> Result<AuthUser, ApiError> {
    let authorization = header(req, "authorization");
    if route.accepts_audio_token() {
        app.authenticate_audio(authorization.as_deref(), query(req, AUDIO_TOKEN_PARAM).as_deref()).await
    } else {
        app.authenticate(authorization.as_deref()).await
    }
}

async fn dispatch(mut req: Request, env: &Env) -> Result<Response, ApiError> {
    let method = req.method().to_string();
    let path = req.path();
    let route = Route::parse(&method, &path).ok_or_else(|| ApiError::not_found("no such API route"))?;
    if route.needs_version_header() {
        check_api_version(header(&req, API_VERSION_HEADER).as_deref())?;
    }
    let app = adapters::app(env)?;
    let user = authenticate(&app, &req, &route).await?;

    match route {
        Route::Session => Ok(respond::json(200, &user.info())),
        Route::AudioToken => Ok(respond::json(200, &app.mint_audio_token(&user)?)),
        Route::DeleteDocument { id } => app.delete_document(&user, &id).await.map(|()| respond::no_content()),
        Route::PutDocumentBody { id } => {
            let target = app.begin_document_body(&user, &id, content_length(&req)).await?;
            let bucket = adapters::media(env)?;
            streams::upload(&req, &bucket, &target.object_key, target.length, "application/json").await?;
            app.finish_document_body(&user, &id).await.map(|()| respond::no_content())
        }
        Route::GetDocumentBody { id } => {
            let key = app.document_body_key(&user, &id).await?;
            let headers = [("content-type", "application/json"), ("cache-control", "private, max-age=86400")];
            streams::download(&adapters::media(env)?, &key, &headers).await
        }
        Route::Voices => Ok(respond::json(200, &app.voice_catalog(&user).await?)),
        Route::CreateVoice => {
            let voice = app.create_voice(&user, body::<CreateVoice>(&mut req).await?).await?;
            Ok(respond::json(201, &voice))
        }
        Route::PutVoiceReference { id } => {
            let target = app.begin_voice_reference(&user, &id, content_length(&req)).await?;
            let bucket = adapters::media(env)?;
            streams::upload(&req, &bucket, &target.object_key, target.length, "audio/wav").await?;
            app.finish_voice_reference(&user, &id).await.map(|()| respond::no_content())
        }
        Route::PrepareVoice { id } => Ok(respond::json(202, &app.prepare_voice(&user, &id).await?)),
        Route::DeleteVoice { id } => app.delete_voice(&user, &id).await.map(|()| respond::no_content()),
        Route::Render => {
            let request = body::<RenderRequest>(&mut req).await?;
            Ok(respond::json(200, &app.render(&user, request).await?))
        }
        Route::RenderStatus => {
            let keys = parse_status_keys(&query(&req, "keys").unwrap_or_default())?;
            Ok(respond::json(200, &app.render_status(&keys).await?))
        }
        Route::Audio { key } => audio(&app, env, &req, &key, method == "HEAD").await,
        Route::Fetch => fetch(&app, &req).await,
        Route::Status => Ok(respond::json(200, &app.system_status(&user).await?)),
    }
}

async fn audio(app: &App, env: &Env, req: &Request, key: &str, head: bool) -> Result<Response, ApiError> {
    let object = app.audio_object(key).await?;
    let headers = [("content-type", AUDIO_CONTENT_TYPE), ("cache-control", AUDIO_CACHE_CONTROL)];
    let range = header(req, "range");
    streams::audio(&adapters::media(env)?, &object.object_key, object.bytes, range.as_deref(), head, &headers).await
}

async fn fetch(app: &App, req: &Request) -> Result<Response, ApiError> {
    let url = query(req, "url").ok_or_else(|| ApiError::bad_request("give the page address as ?url="))?;
    let page = app.fetch_page(&url).await?;
    let content_type = page.content_type();
    Ok(respond::build(
        200,
        &[
            ("content-type", &content_type),
            ("x-upstream-content-type", &page.upstream_type),
            ("x-final-url", &page.final_url),
            ("content-disposition", "attachment"),
            ("cache-control", "no-store"),
        ],
        Some(page.body),
    ))
}

/// `POST /hooks/auth-email`, called by Supabase Auth (not by apps; no CORS, no version header).
pub async fn auth_email_hook(mut req: Request, env: &Env) -> Response {
    let refuse = |e: &HookError| {
        let body = e.body().to_string().into_bytes();
        respond::build(e.http_code, &[("content-type", "application/json"), ("cache-control", "no-store")], Some(body))
    };
    if req.method() != worker::Method::Post {
        return method_not_allowed();
    }
    let body = match bytes_limited(&mut req, MAX_HOOK_BYTES).await {
        Ok(b) => b,
        Err(e) => return refuse(&HookError { http_code: e.status(), message: e.message }),
    };
    let app = match adapters::app(env) {
        Ok(app) => app,
        Err(e) => {
            worker::console_error!("auth-email: {}", e);
            return refuse(&HookError { http_code: 500, message: "email hook unavailable".into() });
        }
    };
    let (id, timestamp, signature) =
        (header(&req, "webhook-id"), header(&req, "webhook-timestamp"), header(&req, "webhook-signature"));
    let headers = HookHeaders {
        id: id.as_deref().unwrap_or_default(),
        timestamp: timestamp.as_deref().unwrap_or_default(),
        signature: signature.as_deref().unwrap_or_default(),
    };
    match app.handle_auth_email(headers, &body).await {
        Ok(outcome) => {
            worker::console_log!("auth-email: {:?}", outcome);
            respond::json(200, &serde_json::json!({}))
        }
        Err(e) => {
            worker::console_error!("auth-email: {} {}", e.http_code, e.message);
            refuse(&e)
        }
    }
}

/// Response for methods other than GET and HEAD on app paths.
pub fn method_not_allowed() -> Response {
    let mut resp = Response::builder().with_status(405).empty();
    let _ = resp.headers_mut().set("allow", "GET, HEAD");
    resp
}
