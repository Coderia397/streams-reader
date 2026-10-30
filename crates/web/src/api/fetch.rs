//! `/api/fetch`: the Worker downloads a web page for import (the browser cannot, because of CORS).

use super::ClientError;
use super::client::{Body, encode_component, read_text, send};

pub struct FetchedPage {
    pub html: String,
    /// URL after redirects (`X-Final-Url`), used to resolve relative links and as the source URL.
    pub final_url: String,
}

pub async fn page(url: &str) -> Result<FetchedPage, ClientError> {
    let resp = send("GET", &format!("/api/fetch?url={}", encode_component(url)), Body::Empty).await?;
    let final_url = resp.headers().get("x-final-url").ok().flatten().unwrap_or_else(|| url.to_owned());
    let html = read_text(&resp).await?;
    Ok(FetchedPage { html, final_url })
}
