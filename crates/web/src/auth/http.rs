//! Supabase Auth REST calls (no JS SDK). Every call sends the publishable key as `apikey`; calls on behalf of the
//! signed-in user add `Authorization: Bearer <access token>`.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use super::error::AuthError;
use super::types::{AuthSettings, Provider, Session, SignUpData, User, provider_id};
use crate::api::client::{encode_component, js_error_text};
use crate::config::{SUPABASE_KEY, SUPABASE_URL};
use crate::util::dom;

async fn call(method: &str, path: &str, body: Option<&Value>, bearer: Option<&str>) -> Result<String, AuthError> {
    let window = dom::window().ok_or_else(|| AuthError::network("no window"))?;
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    init.set_credentials(web_sys::RequestCredentials::Omit);
    let headers = web_sys::Headers::new().map_err(|e| AuthError::network(&js_error_text(e)))?;
    let set = |k: &str, v: &str| headers.set(k, v).map_err(|e| AuthError::network(&js_error_text(e)));
    set("apikey", SUPABASE_KEY)?;
    if let Some(token) = bearer {
        set("authorization", &format!("Bearer {token}"))?;
    }
    if let Some(b) = body {
        set("content-type", "application/json")?;
        init.set_body(&JsValue::from_str(&b.to_string()));
    }
    init.set_headers(&headers);
    let url = format!("{SUPABASE_URL}/auth/v1{path}");
    let resp = JsFuture::from(window.fetch_with_str_and_init(&url, &init))
        .await
        .map_err(|e| AuthError::network(&js_error_text(e)))?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| AuthError::network("not a Response"))?;
    let text = match resp.text() {
        Ok(p) => JsFuture::from(p).await.ok().and_then(|v| v.as_string()).unwrap_or_default(),
        Err(_) => String::new(),
    };
    if resp.ok() { Ok(text) } else { Err(AuthError::from_response(resp.status(), &text)) }
}

async fn call_json<T: DeserializeOwned>(
    method: &str,
    path: &str,
    body: Option<&Value>,
    bearer: Option<&str>,
) -> Result<T, AuthError> {
    let text = call(method, path, body, bearer).await?;
    serde_json::from_str(&text).map_err(|e| AuthError::network(&e.to_string()))
}

/// Where OAuth returns: this origin's `/app/auth/callback`.
pub fn redirect_to() -> String {
    let origin = dom::window().and_then(|w| w.location().origin().ok()).unwrap_or_default();
    format!("{origin}{}/auth/callback", crate::config::BASE)
}

pub async fn settings() -> Result<AuthSettings, AuthError> {
    call_json("GET", "/settings", None, None).await
}

/// Sends the 6-digit code; the same email carries a link to `/app/auth/confirm`. Existing accounts sign in with
/// `create_user = false` (GoTrue answers `otp_disabled` when there is no such account); a new account needs
/// `create_user = true` and the invite code in `data` (checked by the sign-up hook).
pub async fn send_code(email: &str, create_user: bool, data: Option<&SignUpData>) -> Result<(), AuthError> {
    let mut body = json!({ "email": email, "create_user": create_user });
    if let Some(d) = data {
        body["data"] = json!(d);
    }
    call("POST", "/otp", Some(&body), None).await.map(|_| ())
}

pub async fn verify_code(email: &str, token: &str) -> Result<Session, AuthError> {
    let body = json!({ "type": "email", "email": email, "token": token });
    call_json("POST", "/verify", Some(&body), None).await
}

/// The link in the email: `token_hash` and `type` (`magiclink`, `signup`, `email`, `email_change`, `recovery`, `invite`).
pub async fn verify_link(token_hash: &str, kind: &str) -> Result<Session, AuthError> {
    let body = json!({ "type": kind, "token_hash": token_hash });
    call_json("POST", "/verify", Some(&body), None).await
}

/// Stores the interface language on the user, so auth emails follow it.
pub async fn update_locale(access: &str, locale: &str) -> Result<(), AuthError> {
    let body = json!({ "data": { "locale": locale } });
    call("PUT", "/user", Some(&body), Some(access)).await.map(|_| ())
}

pub async fn exchange_code(code: &str, verifier: &str) -> Result<Session, AuthError> {
    let body = json!({ "auth_code": code, "code_verifier": verifier });
    call_json("POST", "/token?grant_type=pkce", Some(&body), None).await
}

pub async fn refresh(refresh_token: &str) -> Result<Session, AuthError> {
    let body = json!({ "refresh_token": refresh_token });
    call_json("POST", "/token?grant_type=refresh_token", Some(&body), None).await
}

pub async fn user(access: &str) -> Result<User, AuthError> {
    call_json("GET", "/user", None, Some(access)).await
}

/// `scope`: `local` (this device) or `global` (every device).
pub async fn logout(access: &str, scope: &str) -> Result<(), AuthError> {
    call("POST", &format!("/logout?scope={scope}"), None, Some(access)).await.map(|_| ())
}

/// The provider's sign-in page (PKCE). Supabase redirects back to [`redirect_to`] with `?code=`.
pub fn authorize_url(provider: Provider, challenge: &str, redirect: &str) -> String {
    format!(
        "{SUPABASE_URL}/auth/v1/authorize?provider={}&redirect_to={}&code_challenge={challenge}&code_challenge_method=s256",
        provider_id(provider),
        encode_component(redirect),
    )
}

#[derive(Deserialize)]
struct LinkAnswer {
    url: String,
}

/// The provider page that links another sign-in method to the current account.
pub async fn link_url(access: &str, provider: Provider, challenge: &str) -> Result<String, AuthError> {
    let path = format!(
        "/user/identities/authorize?provider={}&redirect_to={}&skip_http_redirect=true&code_challenge={challenge}&code_challenge_method=s256",
        provider_id(provider),
        encode_component(&redirect_to()),
    );
    call_json::<LinkAnswer>("GET", &path, None, Some(access)).await.map(|a| a.url)
}

pub async fn unlink(access: &str, identity_id: &str) -> Result<(), AuthError> {
    call("DELETE", &format!("/user/identities/{}", encode_component(identity_id)), None, Some(access)).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_url_carries_pkce_and_the_provider_id() {
        let url = authorize_url(Provider::Azure, "abc", "https://listen.streams.watch/app/auth/callback");
        assert!(url.contains(
            "/auth/v1/authorize?provider=azure&redirect_to=https%3A%2F%2Flisten.streams.watch%2Fapp%2Fauth%2Fcallback&"
        ));
        assert!(url.ends_with("&code_challenge=abc&code_challenge_method=s256"));
    }
}
