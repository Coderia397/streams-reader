//! Large bodies go straight between the client and R2 as JS streams, never through wasm memory: the Workers CPU
//! limit counts every byte copied into Rust.

use nolabs_core::proto::ApiError;
use nolabs_server::services::range::{ByteRange, parse_range};
use worker::wasm_bindgen_futures::JsFuture;
use worker::{Bucket, Data, Headers, HttpMetadata, Range, Request, Response};

use crate::respond::internal;

/// Stream a request body of exactly `length` bytes into R2 at `key`. A `FixedLengthStream` gives R2 the length up
/// front and fails the upload if the client sends more or fewer bytes.
pub async fn upload(
    req: &Request,
    bucket: &Bucket,
    key: &str,
    length: u64,
    content_type: &str,
) -> Result<(), ApiError> {
    let body = req.inner().body().ok_or_else(|| ApiError::bad_request("request body is missing"))?;
    let length = u32::try_from(length).map_err(|_| ApiError::bad_request("body too large"))?;
    let fixed = worker::worker_sys::FixedLengthStream::new(length)
        .map_err(|e| ApiError::internal(format!("FixedLengthStream: {e:?}")))?;
    let pipe = JsFuture::from(body.pipe_to(&fixed.writable()));
    let meta = HttpMetadata { content_type: Some(content_type.to_owned()), ..HttpMetadata::default() };
    let put = bucket.put(key, Data::ReadableStream(fixed.readable())).http_metadata(meta).execute();
    let (put, pipe) = futures_util::future::join(put, pipe).await;
    if pipe.is_err() {
        return Err(ApiError::bad_request("the upload was interrupted or had the wrong length"));
    }
    put.map_err(internal)?;
    Ok(())
}

/// Stream an R2 object as the whole response body.
pub async fn download(bucket: &Bucket, key: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError> {
    let object =
        bucket.get(key).execute().await.map_err(internal)?.ok_or_else(|| ApiError::not_found("object missing"))?;
    let h = Headers::new();
    for (k, v) in headers {
        let _ = h.set(k, v);
    }
    let _ = h.set("content-length", &object.size().to_string());
    let body =
        object.body().ok_or_else(|| ApiError::internal("object without a body"))?.response_body().map_err(internal)?;
    Ok(Response::builder().with_status(200).with_headers(h).body(body))
}

/// Stream audio with single-range support: 200 for the whole object, 206 with `Content-Range` for a range, 416 for
/// a range past the end. `total` is the size recorded when the audio was rendered.
pub async fn audio(
    bucket: &Bucket,
    key: &str,
    total: Option<u64>,
    range_header: Option<&str>,
    head: bool,
    headers: &[(&str, &str)],
) -> Result<Response, ApiError> {
    let range = total.map_or(ByteRange::Full, |t| parse_range(range_header, t));
    let h = Headers::new();
    for (k, v) in headers {
        let _ = h.set(k, v);
    }
    let _ = h.set("accept-ranges", "bytes");
    let total_or_zero = total.unwrap_or(0);
    if range == ByteRange::Unsatisfiable {
        let _ = h.set("content-range", &range.content_range(total_or_zero).unwrap_or_default());
        return Ok(Response::builder().with_status(416).with_headers(h).empty());
    }
    let mut get = bucket.get(key);
    if let ByteRange::Partial { start, end } = range {
        get = get.range(Range::OffsetWithLength { offset: start, length: end - start + 1 });
    }
    let object = get.execute().await.map_err(internal)?.ok_or_else(|| ApiError::not_found("audio missing"))?;
    let size = object.size();
    let (status, length) = match range {
        ByteRange::Partial { .. } => {
            let _ = h.set("content-range", &range.content_range(size).unwrap_or_default());
            (206, range.len(size))
        }
        _ => (200, size),
    };
    let _ = h.set("content-length", &length.to_string());
    let _ = h.set("etag", &object.http_etag());
    let builder = Response::builder().with_status(status).with_headers(h);
    if head {
        return Ok(builder.empty());
    }
    let body =
        object.body().ok_or_else(|| ApiError::internal("object without a body"))?.response_body().map_err(internal)?;
    Ok(builder.body(body))
}
