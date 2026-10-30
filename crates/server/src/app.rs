//! The service container: every port plus the configuration. Each service module adds its methods to [`App`] in its
//! own `impl` block, so a route handler only needs one value.

use nolabs_core::ids;
use nolabs_core::proto::ApiError;

use crate::config::Config;
use crate::ports::{Blob, Clock, Db, Http, Queue, Rng};
use crate::repo::Repo;
use crate::space::SpaceClient;

pub struct App {
    pub db: Box<dyn Db>,
    pub blob: Box<dyn Blob>,
    pub http: Box<dyn Http>,
    pub queue: Box<dyn Queue>,
    pub clock: Box<dyn Clock>,
    pub rng: Box<dyn Rng>,
    pub config: Config,
}

impl App {
    pub fn repo(&self) -> Repo<'_> {
        Repo::new(self.db.as_ref())
    }

    pub fn space(&self) -> SpaceClient<'_> {
        SpaceClient::new(self.http.as_ref(), &self.config.space_url, &self.config.hf_token)
    }

    pub fn now_ms(&self) -> i64 {
        self.clock.now_ms()
    }

    /// A new random id ([`nolabs_core::ids`]).
    pub fn new_id(&self) -> Result<String, ApiError> {
        let mut bytes = [0u8; 10];
        self.rng.fill(&mut bytes)?;
        Ok(ids::id_from_bytes(bytes))
    }
}
