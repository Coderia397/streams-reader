//! `/api/status`: Space stage, GPU budget and queue length for the settings screen and the voice sheet.

use nolabs_core::proto::api::SystemStatus;

use super::ClientError;
use super::client::get_json;

pub async fn status() -> Result<SystemStatus, ClientError> {
    get_json("/api/status").await
}
