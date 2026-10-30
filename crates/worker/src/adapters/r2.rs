//! [`Blob`] over the R2 binding (small objects only; large bodies are streamed in `crate::streams`).

use async_trait::async_trait;
use nolabs_server::ports::{Blob, PortError};
use worker::{Bucket, Data, HttpMetadata};

pub struct R2Blob(pub Bucket);

/// R2 deletes at most 1000 keys per call.
const DELETE_CHUNK: usize = 1000;

fn port(e: worker::Error) -> PortError {
    PortError::new(format!("R2: {e}"))
}

#[async_trait(?Send)]
impl Blob for R2Blob {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PortError> {
        let Some(object) = self.0.get(key).execute().await.map_err(port)? else {
            return Ok(None);
        };
        match object.body() {
            Some(body) => Ok(Some(body.bytes().await.map_err(port)?)),
            None => Ok(Some(Vec::new())),
        }
    }

    async fn put(&self, key: &str, body: Vec<u8>, content_type: &str) -> Result<(), PortError> {
        let meta = HttpMetadata { content_type: Some(content_type.to_owned()), ..HttpMetadata::default() };
        self.0.put(key, Data::Bytes(body)).http_metadata(meta).execute().await.map_err(port)?;
        Ok(())
    }

    async fn head(&self, key: &str) -> Result<Option<u64>, PortError> {
        Ok(self.0.head(key).await.map_err(port)?.map(|o| o.size()))
    }

    async fn delete_many(&self, keys: &[String]) -> Result<(), PortError> {
        for chunk in keys.chunks(DELETE_CHUNK) {
            self.0.delete_multiple(chunk.iter().map(String::as_str).collect()).await.map_err(port)?;
        }
        Ok(())
    }

    async fn list_prefix(&self, prefix: &str) -> Result<Vec<String>, PortError> {
        let mut keys = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut list = self.0.list().prefix(prefix);
            if let Some(c) = cursor.take() {
                list = list.cursor(c);
            }
            let page = list.execute().await.map_err(port)?;
            keys.extend(page.objects().iter().map(|o| o.key()));
            match page.cursor().filter(|_| page.truncated()) {
                Some(c) => cursor = Some(c),
                None => return Ok(keys),
            }
        }
    }
}
