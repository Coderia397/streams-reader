//! `setup`: ensures the D1 database, the render Queue and the R2 bucket exist and that the bucket is private (no
//! r2.dev URL, no custom domain, no CORS) with the Kokoro audio lifecycle rule. Idempotent; records ids in
//! `deploy/state.toml`.

use anyhow::{Result, bail};
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::cf::{Body, Cf};
use crate::cmd::resources::{ensure_d1, ensure_queue};
use crate::lifecycle;
use crate::names::BUCKET;
use crate::paths;
use crate::state::{self, State};

#[derive(Deserialize)]
struct ManagedDomain {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    domain: Option<String>,
}

#[derive(Deserialize)]
struct CustomDomains {
    #[serde(default)]
    domains: Vec<CustomDomain>,
}

#[derive(Deserialize)]
struct CustomDomain {
    domain: String,
}

#[derive(Deserialize)]
struct Rules {
    #[serde(default)]
    rules: Vec<Value>,
}

pub fn run() -> Result<()> {
    let cf = Cf::from_deploy_env()?;

    let (d1, created) = ensure_d1(&cf)?;
    println!("D1 {}: {} ({})", d1.name, d1.uuid, created_word(created));

    let (queue, created) = ensure_queue(&cf)?;
    println!("Queue {}: {} ({})", queue.queue_name, queue.queue_id, created_word(created));

    ensure_bucket(&cf)?;

    let path = paths::state_file();
    let mut st = State::load(&path)?;
    st.d1 = Some(state::Named { name: d1.name, id: d1.uuid });
    st.queue = Some(state::Named { name: queue.queue_name, id: queue.queue_id });
    st.r2 = Some(state::Bucket { name: BUCKET.to_owned(), lifecycle_rule: lifecycle::RULE_ID.to_owned() });
    st.save(&path)?;
    println!("recorded in {}", paths::tilde(&path));
    Ok(())
}

fn ensure_bucket(cf: &Cf) -> Result<()> {
    let bucket_path = cf.acct(&format!("/r2/buckets/{BUCKET}"));
    if cf.get_opt::<Value>(&bucket_path)?.is_none() {
        let _: Value =
            cf.mutate(BUCKET, Method::POST, &cf.acct("/r2/buckets"), Body::Json(json!({ "name": BUCKET })))?;
        println!("R2 {BUCKET}: created");
    } else {
        println!("R2 {BUCKET}: exists");
    }

    let managed_path = format!("{bucket_path}/domains/managed");
    let managed: ManagedDomain = cf.get(&managed_path)?;
    if managed.enabled {
        let _: Value = cf.mutate(BUCKET, Method::PUT, &managed_path, Body::Json(json!({ "enabled": false })))?;
        println!("R2 {BUCKET}: disabled public r2.dev access ({})", managed.domain.unwrap_or_default());
    } else {
        println!("R2 {BUCKET}: r2.dev access off");
    }

    let custom: Option<CustomDomains> = cf.get_opt(&format!("{bucket_path}/domains/custom"))?;
    let custom: Vec<String> = custom.map(|c| c.domains.into_iter().map(|d| d.domain).collect()).unwrap_or_default();
    if !custom.is_empty() {
        // Removing a domain touches DNS; leave that decision to a person.
        bail!("R2 {BUCKET} is public through custom domain(s) {custom:?}; remove them in the dashboard");
    }

    let cors_path = format!("{bucket_path}/cors");
    match cf.get_opt::<Rules>(&cors_path)? {
        Some(cors) if !cors.rules.is_empty() => {
            let _: Value = cf.mutate(BUCKET, Method::DELETE, &cors_path, Body::Empty)?;
            println!("R2 {BUCKET}: removed {} CORS rule(s)", cors.rules.len());
        }
        _ => println!("R2 {BUCKET}: no CORS"),
    }

    let lifecycle_path = format!("{bucket_path}/lifecycle");
    let existing: Rules = cf.get_opt(&lifecycle_path)?.unwrap_or(Rules { rules: vec![] });
    match lifecycle::merge(&existing.rules) {
        Some(rules) => {
            let _: Value = cf.mutate(BUCKET, Method::PUT, &lifecycle_path, Body::Json(json!({ "rules": rules })))?;
            println!("R2 {BUCKET}: lifecycle rule {} set", lifecycle::RULE_ID);
        }
        None => println!("R2 {BUCKET}: lifecycle rule {} current", lifecycle::RULE_ID),
    }
    Ok(())
}

fn created_word(created: bool) -> &'static str {
    if created { "created" } else { "exists" }
}
