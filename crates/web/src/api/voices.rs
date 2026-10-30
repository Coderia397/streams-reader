//! `/api/voices`: catalogue, creating a cloned voice (metadata, WAV upload, prepare) and deleting it.

use nolabs_core::proto::api::{CreateVoice, VoiceCatalog, VoiceInfo};

use super::ClientError;
use super::client::{Body, encode_segment, get_json, read_json, send, send_json, send_no_content};

fn voice_path(id: &str, rest: &str) -> String {
    format!("/api/voices/{}{rest}", encode_segment(id))
}

pub async fn catalog() -> Result<VoiceCatalog, ClientError> {
    get_json("/api/voices").await
}

pub async fn create(req: &CreateVoice) -> Result<VoiceInfo, ClientError> {
    send_json("POST", "/api/voices", req).await
}

pub async fn put_reference(id: &str, wav: &[u8]) -> Result<(), ClientError> {
    let data = js_sys::Uint8Array::from(wav);
    send_no_content("PUT", &voice_path(id, "/reference"), Body::Bytes { data, content_type: "audio/wav" }).await
}

pub async fn prepare(id: &str) -> Result<VoiceInfo, ClientError> {
    let resp = send("POST", &voice_path(id, "/prepare"), Body::Empty).await?;
    read_json(&resp).await
}

pub async fn delete(id: &str) -> Result<(), ClientError> {
    send_no_content("DELETE", &voice_path(id, ""), Body::Empty).await
}
