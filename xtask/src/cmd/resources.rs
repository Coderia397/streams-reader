//! Find-by-name lookups for the D1 database and the Queue. Ids are always resolved from the API by name before they
//! are used in a mutating path, so a stale `deploy/state.toml` can never point a write at someone else's resource.

use anyhow::{Result, bail};
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::cf::{Body, Cf};
use crate::names::{D1_DATABASE, QUEUE};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct D1 {
    pub uuid: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Queue {
    pub queue_id: String,
    pub queue_name: String,
}

/// D1 primary location. Assumed: the user is in the UK, so western Europe keeps writes close.
const D1_LOCATION_HINT: &str = "weur";

pub fn find_d1(cf: &Cf) -> Result<Option<D1>> {
    let all: Vec<D1> = cf.get_all(&cf.acct(&format!("/d1/database?name={D1_DATABASE}")))?;
    exactly_one(all.into_iter().filter(|d| d.name == D1_DATABASE).collect(), D1_DATABASE)
}

pub fn require_d1(cf: &Cf) -> Result<D1> {
    find_d1(cf)?.ok_or_else(|| anyhow::anyhow!("D1 database {D1_DATABASE} not found: run `cargo xtask setup` first"))
}

pub fn ensure_d1(cf: &Cf) -> Result<(D1, bool)> {
    if let Some(d) = find_d1(cf)? {
        return Ok((d, false));
    }
    let body = json!({ "name": D1_DATABASE, "primary_location_hint": D1_LOCATION_HINT });
    let created: D1 = cf.mutate(D1_DATABASE, Method::POST, &cf.acct("/d1/database"), Body::Json(body))?;
    Ok((created, true))
}

pub fn find_queue(cf: &Cf) -> Result<Option<Queue>> {
    let all: Vec<Queue> = cf.get_all(&cf.acct("/queues"))?;
    exactly_one(all.into_iter().filter(|q| q.queue_name == QUEUE).collect(), QUEUE)
}

pub fn require_queue(cf: &Cf) -> Result<Queue> {
    find_queue(cf)?.ok_or_else(|| anyhow::anyhow!("queue {QUEUE} not found: run `cargo xtask setup` first"))
}

pub fn ensure_queue(cf: &Cf) -> Result<(Queue, bool)> {
    if let Some(q) = find_queue(cf)? {
        return Ok((q, false));
    }
    let created: Queue =
        cf.mutate(QUEUE, Method::POST, &cf.acct("/queues"), Body::Json(json!({ "queue_name": QUEUE })))?;
    Ok((created, true))
}

/// Runs one SQL request against the `nolabs` database and returns the rows of each statement.
pub fn d1_query(cf: &Cf, db: &D1, sql: &str) -> Result<Vec<Vec<Value>>> {
    #[derive(Deserialize)]
    struct StatementResult {
        #[serde(default)]
        results: Vec<Value>,
        #[serde(default = "yes")]
        success: bool,
    }
    fn yes() -> bool {
        true
    }
    let path = cf.acct(&format!("/d1/database/{}/query", db.uuid));
    let out: Vec<StatementResult> = cf.mutate(&db.name, Method::POST, &path, Body::Json(json!({ "sql": sql })))?;
    if out.iter().any(|s| !s.success) {
        bail!("D1 reported a failed statement");
    }
    Ok(out.into_iter().map(|s| s.results).collect())
}

fn exactly_one<T>(mut found: Vec<T>, name: &str) -> Result<Option<T>> {
    match found.len() {
        0 => Ok(None),
        1 => Ok(found.pop()),
        n => bail!("{n} resources are named {name}; resolve the duplicate in the dashboard"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn exactly_one_rules() {
        assert_eq!(exactly_one(Vec::<u8>::new(), "x").unwrap(), None);
        assert_eq!(exactly_one(vec![1], "x").unwrap(), Some(1));
        assert!(exactly_one(vec![1, 2], "x").is_err());
    }

    #[test]
    fn parses_list_items() {
        let d: D1 =
            serde_json::from_value(json!({"uuid":"u","name":"nolabs","version":"production","num_tables":0})).unwrap();
        assert_eq!(d.uuid, "u");
        let q: Queue =
            serde_json::from_value(json!({"queue_id":"q","queue_name":"nolabs-render","consumers":[]})).unwrap();
        assert_eq!(q.queue_id, "q");
    }
}
