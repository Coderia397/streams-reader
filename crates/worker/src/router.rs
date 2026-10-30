//! The `/api` dispatcher: route table and guards from `nolabs_server::routes`, then one handler per route. JSON
//! routes call the services; body, audio and recording routes stream between the client and R2.

use nolabs_core::proto::ApiError;
use nolabs_core::proto::api::{API_VERSION_HEADER, CreateDocument, CreateVoice, LoginRequest, Progress, RenderRequest};
use nolabs_server::App;
use nolabs_server::auth::guard::{check_api_version, check_same_origin};
use nolabs_server::error::{parse_json, too_large};
use nolabs_server::routes::Route;
use nolabs_server::services::audio::{AUDIO_CACHE_CONTROL, AUDIO_CONTENT_TYPE};
use nolabs_server::services::render::parse_status_keys;
use worker::{Env, Request, Response};

use crate::adapters;
use crate::respond::{self, internal};
use crate::streams;

/// JSON request bodies are small (a render request is at most 64 items of 1000 chars).
const MAX_JSON_BYTES: u64 = 1024 * 1024;

fn header(req: &Request, name: &str) -> Option<String> {
    req.headers().get(name).ok().flatten()
}

fn content_length(req: &Request) -> Option<u64> {
    header(req, "content-length").and_then(|v| v.trim().parse().ok())
}

async fn body<T: serde::de::DeserializeOwned>(req: &mut Request) -> Result<T, ApiError> {
    if content_length(req).is_some_and(|n| n > MAX_JSON_BYTES) {
        return Err(too_large("request body too large"));
    }
    let bytes = req.bytes().await.map_err(internal)?;
    if bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(too_large("request body too large"));
    }
    parse_json(&bytes)
}

fn query(req: &Request, name: &str) -> Option<String> {
    req.url().ok()?.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned())
}

/// Handle one API request; every failure becomes the JSON error body.
pub async fn api(req: Request, env: &Env) -> Response {
    let mut refreshed: Option<String> = None;
    let mut resp = match dispatch(req, env, &mut refreshed).await {
        Ok(resp) => resp,
        Err(e) => {
            if e.status() >= 500 {
                worker::console_error!("api error: {}", e);
            }
            respond::error(&e)
        }
    };
    if let Some(cookie) = refreshed {
        let _ = resp.headers_mut().append("set-cookie", &cookie);
    }
    resp
}

async fn dispatch(mut req: Request, env: &Env, refreshed: &mut Option<String>) -> Result<Response, ApiError> {
    let method = req.method().to_string();
    let path = req.path();
    let route = Route::parse(&method, &path).ok_or_else(|| ApiError::not_found("no such API route"))?;

    let url = req.url().map_err(internal)?;
    let own_origin = url.origin().ascii_serialization();
    check_same_origin(
        &method,
        header(&req, "sec-fetch-site").as_deref(),
        header(&req, "origin").as_deref(),
        &own_origin,
    )?;
    if route.needs_version_header() {
        check_api_version(header(&req, API_VERSION_HEADER).as_deref())?;
    }

    let app = adapters::app(env)?;
    let cookie = header(&req, "cookie");
    let session = if route.needs_session() { Some(app.authenticate(cookie.as_deref()).await?) } else { None };
    if let Some(s) = &session {
        refreshed.clone_from(&s.refreshed_cookie);
    }

    match route {
        Route::Login => {
            let set_cookie = app.login(&body::<LoginRequest>(&mut req).await?).await?;
            let mut resp = respond::no_content();
            let _ = resp.headers_mut().set("set-cookie", &set_cookie);
            Ok(resp)
        }
        Route::Logout => {
            let clear = app.logout(cookie.as_deref()).await?;
            let mut resp = respond::no_content();
            let _ = resp.headers_mut().set("set-cookie", &clear);
            Ok(resp)
        }
        Route::Session => {
            let info = session.map(|s| s.info()).ok_or_else(|| ApiError::internal("session missing"))?;
            Ok(respond::json(200, &info))
        }
        Route::Library => Ok(respond::json(200, &app.library().await?)),
        Route::CreateDocument => {
            let doc = app.create_document(body::<CreateDocument>(&mut req).await?).await?;
            Ok(respond::json(201, &doc))
        }
        Route::GetDocument { id } => Ok(respond::json(200, &app.get_document(&id).await?)),
        Route::DeleteDocument { id } => app.delete_document(&id).await.map(|()| respond::no_content()),
        Route::PutDocumentBody { id } => {
            let target = app.begin_document_body(&id, content_length(&req)).await?;
            let bucket = adapters::media(env)?;
            streams::upload(&req, &bucket, &target.object_key, target.length, "application/json").await?;
            app.finish_document_body(&id).await.map(|()| respond::no_content())
        }
        Route::GetDocumentBody { id } => {
            let key = app.document_body_key(&id).await?;
            let headers = [("content-type", "application/json"), ("cache-control", "private, max-age=86400")];
            streams::download(&adapters::media(env)?, &key, &headers).await
        }
        Route::PutProgress { id, .. } => {
            // Beacons arrive as text/plain; the body is JSON either way.
            let progress = body::<Progress>(&mut req).await?;
            app.put_progress(&id, progress).await.map(|()| respond::no_content())
        }
        Route::Voices => Ok(respond::json(200, &app.voice_catalog().await?)),
        Route::CreateVoice => Ok(respond::json(201, &app.create_voice(body::<CreateVoice>(&mut req).await?).await?)),
        Route::PutVoiceReference { id } => {
            let target = app.begin_voice_reference(&id, content_length(&req)).await?;
            let bucket = adapters::media(env)?;
            streams::upload(&req, &bucket, &target.object_key, target.length, "audio/wav").await?;
            app.finish_voice_reference(&id).await.map(|()| respond::no_content())
        }
        Route::PrepareVoice { id } => Ok(respond::json(202, &app.prepare_voice(&id).await?)),
        Route::DeleteVoice { id } => app.delete_voice(&id).await.map(|()| respond::no_content()),
        Route::Render => Ok(respond::json(200, &app.render(body::<RenderRequest>(&mut req).await?).await?)),
        Route::RenderStatus => {
            let keys = parse_status_keys(&query(&req, "keys").unwrap_or_default())?;
            Ok(respond::json(200, &app.render_status(&keys).await?))
        }
        Route::Audio { key } => audio(&app, env, &req, &key, method == "HEAD").await,
        Route::Fetch => fetch(&app, &req).await,
        Route::Status => Ok(respond::json(200, &app.system_status().await?)),
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

/// Response for methods other than GET and HEAD on app paths.
pub fn method_not_allowed() -> Response {
    let mut resp = Response::builder().with_status(405).empty();
    let _ = resp.headers_mut().set("allow", "GET, HEAD");
    resp
}
