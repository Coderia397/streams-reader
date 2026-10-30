//! Test support: SQLite-backed [`crate::ports::Db`], in-memory fakes and a ready-made [`App`].

pub mod fakes;
pub mod sqlite;

use std::cell::Cell;
use std::rc::Rc;

use nolabs_core::budget::GpuPolicy;

use crate::app::App;
use crate::auth::AuthUser;
use crate::auth::jwks::JwksCache;
use crate::auth::jwt::testing::{jwks, signing_key};
use crate::config::Config;
use fakes::{FakeAccounts, FakeBlob, FakeClock, FakeHttp, FakeQueue, FakeRng, response};
use sqlite::SqliteDb;

pub const AUDIO_URL_KEY: [u8; 32] = [1; 32];
/// 2026-10-30T12:00:00Z.
pub const T0: i64 = 1_793_448_000_000;
pub const SUPABASE_URL: &str = "https://ref.supabase.co";

pub fn test_config() -> Config {
    Config {
        space_url: "https://space.test".into(),
        space_repo: "owner/space".into(),
        kokoro_rev: "hexgrad/Kokoro-82M@abc".into(),
        ming_rev: "inclusionAI/Ming@def".into(),
        app_version: "test".into(),
        hf_token: "hf_test".into(),
        supabase_url: SUPABASE_URL.into(),
        supabase_anon_key: "sb_publishable_test".into(),
        supabase_jwt_secret: None,
        audio_url_key: AUDIO_URL_KEY.to_vec(),
        send_email_hook_secret: Some("v1,whsec_c2VjcmV0LWtleS1mb3ItdGVzdGluZy1vbmx5".into()),
        brevo_api_key: Some("brevo-test-key".into()),
        site_url: "https://listen.streams.watch".into(),
        gpu: GpuPolicy::default(),
    }
}

pub struct TestEnv {
    pub app: App,
    pub db: SqliteDb,
    pub blob: FakeBlob,
    pub http: FakeHttp,
    pub queue: FakeQueue,
    pub accounts: FakeAccounts,
    clock: Rc<Cell<i64>>,
}

impl TestEnv {
    pub fn new() -> TestEnv {
        let db = SqliteDb::migrated();
        let blob = FakeBlob::default();
        let http = FakeHttp::default();
        let queue = FakeQueue::default();
        let accounts = FakeAccounts::default();
        let clock = Rc::new(Cell::new(T0));
        let app = App {
            db: Box::new(db.clone()),
            blob: Box::new(blob.clone()),
            http: Box::new(http.clone()),
            queue: Box::new(queue.clone()),
            clock: Box::new(FakeClock(clock.clone())),
            rng: Box::new(FakeRng::default()),
            accounts: Box::new(accounts.clone()),
            jwks: Rc::new(JwksCache::new()),
            config: test_config(),
        };
        TestEnv { app, db, blob, http, queue, accounts, clock }
    }

    pub fn now(&self) -> i64 {
        self.clock.get()
    }

    pub fn advance(&self, ms: i64) {
        self.clock.set(self.clock.get() + ms);
    }

    /// A signed-in caller whose access token the accounts fake maps back to `id`.
    pub fn user(&self, id: &str) -> AuthUser {
        AuthUser {
            id: id.to_owned(),
            email: None,
            expires_at_ms: self.now() + 3_600_000,
            access_token: Some(FakeAccounts::token_for(id)),
        }
    }

    /// Serve a JWKS with the test keys `(seed, kid)` (see `jwt::testing::signing_key`) from the project URL.
    pub fn set_jwks(&self, keys: &[(u8, &str)]) {
        let owned: Vec<_> = keys.iter().map(|(seed, kid)| (signing_key(*seed), (*kid).to_owned())).collect();
        let refs: Vec<_> = owned.iter().map(|(k, kid)| (k, kid.as_str())).collect();
        let body = jwks(&refs);
        let url = format!("{SUPABASE_URL}/auth/v1/.well-known/jwks.json");
        self.http.set(
            move |req| {
                if req.url == url { Ok(response(200, &[], body.clone())) } else { Ok(response(404, &[], "")) }
            },
        );
    }

    pub fn jwks_fetches(&self) -> usize {
        self.http.requests.borrow().iter().filter(|r| r.url.ends_with("/jwks.json")).count()
    }
}

/// Run a future to completion on the current thread.
pub fn run<F: std::future::Future>(f: F) -> F::Output {
    futures::executor::block_on(f)
}
