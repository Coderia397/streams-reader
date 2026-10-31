//! The I/O the services need, as small object-safe traits. The Worker implements them over D1, R2, Queues, `fetch`,
//! `Date.now()` and `crypto.getRandomValues`; the tests implement them over SQLite and in-memory fakes. Everything is
//! `?Send` because the Worker runtime is single-threaded and its JS handles are not `Send`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A failure inside an adapter (binding error, network error, malformed row). Services turn it into a 500.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct PortError(pub String);

impl PortError {
    pub fn new(message: impl Into<String>) -> Self {
        PortError(message.into())
    }
}

// --------------------------------------------------------------------------------------------------------------- database

/// A bound SQL parameter. D1 has no 64-bit integers on the JS side, but every integer we store (millisecond times,
/// byte counts) stays far below 2^53.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

impl From<i64> for SqlValue {
    fn from(v: i64) -> Self {
        SqlValue::Int(v)
    }
}

impl From<u32> for SqlValue {
    fn from(v: u32) -> Self {
        SqlValue::Int(i64::from(v))
    }
}

impl From<u64> for SqlValue {
    fn from(v: u64) -> Self {
        SqlValue::Int(i64::try_from(v).unwrap_or(i64::MAX))
    }
}

impl From<bool> for SqlValue {
    fn from(v: bool) -> Self {
        SqlValue::Int(i64::from(v))
    }
}

impl From<&str> for SqlValue {
    fn from(v: &str) -> Self {
        SqlValue::Text(v.to_owned())
    }
}

impl From<String> for SqlValue {
    fn from(v: String) -> Self {
        SqlValue::Text(v)
    }
}

impl<T: Into<SqlValue>> From<Option<T>> for SqlValue {
    fn from(v: Option<T>) -> Self {
        v.map_or(SqlValue::Null, Into::into)
    }
}

/// One statement with positional `?` parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub sql: String,
    pub params: Vec<SqlValue>,
}

impl Stmt {
    pub fn new(sql: impl Into<String>, params: Vec<SqlValue>) -> Self {
        Stmt { sql: sql.into(), params }
    }
}

/// A result row: column name to JSON value (D1 returns rows as plain JS objects).
pub type Row = serde_json::Map<String, serde_json::Value>;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StmtResult {
    pub rows: Vec<Row>,
    /// Rows changed by an INSERT, UPDATE or DELETE.
    pub changes: u64,
}

#[async_trait(?Send)]
pub trait Db {
    /// Run a statement and return every row it produces (SELECT, or any statement with RETURNING).
    async fn all(&self, stmt: &Stmt) -> Result<Vec<Row>, PortError>;
    /// Run a statement and return how many rows it changed.
    async fn run(&self, stmt: &Stmt) -> Result<u64, PortError>;
    /// Run statements in order inside one transaction: all of them apply or none do.
    async fn batch(&self, stmts: &[Stmt]) -> Result<Vec<StmtResult>, PortError>;
}

// ------------------------------------------------------------------------------------------------------------ blob store

/// Small-object access to R2. Large bodies (document bodies, audio, recordings) are streamed by the Worker itself and
/// never pass through this trait.
#[async_trait(?Send)]
pub trait Blob {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PortError>;
    async fn put(&self, key: &str, body: Vec<u8>, content_type: &str) -> Result<(), PortError>;
    /// Size in bytes, or None when the object does not exist.
    async fn head(&self, key: &str) -> Result<Option<u64>, PortError>;
    /// Delete every listed key; missing keys are not an error.
    async fn delete_many(&self, keys: &[String]) -> Result<(), PortError>;
    /// Every key starting with `prefix` (all pages).
    async fn list_prefix(&self, prefix: &str) -> Result<Vec<String>, PortError>;
}

// ------------------------------------------------------------------------------------------------------------------ http

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    /// PostgREST updates.
    Patch,
    /// PostgREST deletes.
    Delete,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }
}

/// An outgoing request. Adapters never follow redirects (3xx responses are returned as they are), never attach
/// cookies or credentials beyond `headers`, and enforce `timeout_ms` and `max_body_bytes`.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    /// Header names in lowercase.
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    /// Deadline for the whole exchange (`send`) or until the response headers arrive (`open_stream`).
    pub timeout_ms: u32,
    /// Largest body `send` may buffer; larger bodies fail with [`HttpError::TooLarge`].
    pub max_body_bytes: u64,
}

impl HttpRequest {
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        HttpRequest {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: None,
            timeout_ms: 30_000,
            max_body_bytes: 1024 * 1024,
        }
    }

    pub fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.to_ascii_lowercase(), value.into()));
        self
    }

    pub fn json_body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self.header("content-type", "application/json")
    }

    pub fn timeout_ms(mut self, ms: u32) -> Self {
        self.timeout_ms = ms;
        self
    }

    pub fn max_body_bytes(mut self, bytes: u64) -> Self {
        self.max_body_bytes = bytes;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    /// Header names in lowercase.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        header_value(&self.headers, name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HttpError {
    #[error("timed out")]
    Timeout,
    #[error("response body too large")]
    TooLarge,
    #[error("network error: {0}")]
    Network(String),
}

/// A response body read chunk by chunk (the Gradio SSE stream). Chunk boundaries are arbitrary.
#[async_trait(?Send)]
pub trait ByteStream {
    /// The next chunk, or None at the end of the body.
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, HttpError>;
}

pub struct StreamResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Box<dyn ByteStream>,
}

#[async_trait(?Send)]
pub trait Http {
    /// Send a request and buffer the whole response body.
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, HttpError>;
    /// Send a request and return as soon as the headers arrive; the body is read incrementally.
    async fn open_stream(&self, req: HttpRequest) -> Result<StreamResponse, HttpError>;
}

/// Case-insensitive header lookup over a lowercase-named list.
pub fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

// ----------------------------------------------------------------------------------------------------------------- queue

/// The body of every `nolabs-render` queue message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobMessage {
    pub job_id: String,
}

#[async_trait(?Send)]
pub trait Queue {
    async fn send(&self, message: &JobMessage, delay_s: u32) -> Result<(), PortError>;
}

// ------------------------------------------------------------------------------------------------------- clock and rng

pub trait Clock {
    /// Unix time in milliseconds.
    fn now_ms(&self) -> i64;
}

pub trait Rng {
    /// Fill `buf` with cryptographically secure random bytes.
    fn fill(&self, buf: &mut [u8]) -> Result<(), PortError>;
}
