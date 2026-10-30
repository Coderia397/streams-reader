//! [`Queue`] over the `RENDER_QUEUE` producer binding.

use async_trait::async_trait;
use nolabs_server::ports::{JobMessage, PortError, Queue};
use worker::MessageBuilder;

pub struct RenderQueue(pub worker::Queue);

/// Cloudflare Queues accept delays up to 12 hours.
const MAX_DELAY_S: u32 = 12 * 60 * 60;

#[async_trait(?Send)]
impl Queue for RenderQueue {
    async fn send(&self, message: &JobMessage, delay_s: u32) -> Result<(), PortError> {
        let msg = MessageBuilder::new(message.clone()).delay_seconds(delay_s.min(MAX_DELAY_S)).build();
        self.0.send(msg).await.map_err(|e| PortError::new(format!("queue: {e}")))
    }
}
