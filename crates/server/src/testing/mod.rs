//! Test support: SQLite-backed [`crate::ports::Db`], in-memory fakes and a ready-made [`App`].

pub mod fakes;
pub mod sqlite;

use std::cell::Cell;
use std::rc::Rc;

use hmac::{Hmac, Mac};
use nolabs_core::budget::GpuPolicy;
use sha2::Sha256;

use crate::app::App;
use crate::config::Config;
use fakes::{FakeBlob, FakeClock, FakeHttp, FakeQueue, FakeRng};
use sqlite::SqliteDb;

pub const SESSION_KEY: [u8; 32] = [1; 32];
pub const PASSPHRASE: &str = "secret words";
/// 2026-10-30T12:00:00Z.
pub const T0: i64 = 1_793_448_000_000;

pub fn passphrase_hash(passphrase: &str) -> Vec<u8> {
    let mut m = Hmac::<Sha256>::new_from_slice(&SESSION_KEY).unwrap();
    m.update(passphrase.as_bytes());
    m.finalize().into_bytes().to_vec()
}

pub fn test_config() -> Config {
    Config {
        space_url: "https://space.test".into(),
        space_repo: "owner/space".into(),
        kokoro_rev: "hexgrad/Kokoro-82M@abc".into(),
        ming_rev: "inclusionAI/Ming@def".into(),
        app_version: "test".into(),
        hf_token: "hf_test".into(),
        session_key: SESSION_KEY.to_vec(),
        passphrase_hash: passphrase_hash(PASSPHRASE),
        gpu: GpuPolicy::default(),
    }
}

pub struct TestEnv {
    pub app: App,
    pub db: SqliteDb,
    pub blob: FakeBlob,
    pub http: FakeHttp,
    pub queue: FakeQueue,
    clock: Rc<Cell<i64>>,
}

impl TestEnv {
    pub fn new() -> TestEnv {
        let db = SqliteDb::migrated();
        let blob = FakeBlob::default();
        let http = FakeHttp::default();
        let queue = FakeQueue::default();
        let clock = Rc::new(Cell::new(T0));
        let app = App {
            db: Box::new(db.clone()),
            blob: Box::new(blob.clone()),
            http: Box::new(http.clone()),
            queue: Box::new(queue.clone()),
            clock: Box::new(FakeClock(clock.clone())),
            rng: Box::new(FakeRng::default()),
            config: test_config(),
        };
        TestEnv { app, db, blob, http, queue, clock }
    }

    pub fn now(&self) -> i64 {
        self.clock.get()
    }

    pub fn advance(&self, ms: i64) {
        self.clock.set(self.clock.get() + ms);
    }
}

/// Run a future to completion on the current thread.
pub fn run<F: std::future::Future>(f: F) -> F::Output {
    futures::executor::block_on(f)
}
