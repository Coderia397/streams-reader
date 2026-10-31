//! PDF text through pdf.js (`interop/pdf.js`), shaped as the import worker's `PdfLine` pages.

use js_sys::{Array, Uint8Array};
use nolabs_web_import::PdfLine;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::prop;
use crate::api::client::js_error_text;

#[wasm_bindgen(module = "/interop/pdf.js")]
extern "C" {
    #[wasm_bindgen(js_name = extractPdf)]
    fn extract_pdf(bytes: Uint8Array) -> js_sys::Promise;
}

pub struct PdfText {
    pub title: Option<String>,
    pub pages: Vec<Vec<PdfLine>>,
}

fn line(v: &JsValue) -> Option<PdfLine> {
    Some(PdfLine {
        text: prop(v, "text").as_string()?,
        y: prop(v, "y").as_f64().unwrap_or(0.0) as f32,
        height: prop(v, "height").as_f64().unwrap_or(0.0) as f32,
    })
}

pub async fn extract(bytes: &[u8]) -> Result<PdfText, String> {
    let result = JsFuture::from(extract_pdf(Uint8Array::from(bytes))).await.map_err(js_error_text)?;
    let title = prop(&result, "title").as_string().filter(|t| !t.trim().is_empty());
    let pages: Array = prop(&result, "pages").dyn_into().map_err(|_| "pdf.js returned no pages".to_owned())?;
    let pages = pages
        .iter()
        .map(|p| p.dyn_into::<Array>().map(|lines| lines.iter().filter_map(|l| line(&l)).collect()).unwrap_or_default())
        .collect();
    Ok(PdfText { title, pages })
}
