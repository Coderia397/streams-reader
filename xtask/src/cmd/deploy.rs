//! `deploy`: uploads the Worker built by `cargo xtask build`, sets its secrets, attaches it as the render Queue's
//! consumer, sets the cron schedule, enables its workers.dev URL and records the result in `deploy/state.toml`.

use std::fs;

use anyhow::{Context as _, Result, bail};
use reqwest::Method;
use reqwest::blocking::multipart::{Form, Part};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::cf::{Body, Cf};
use crate::cmd::git_short_sha;
use crate::cmd::resources::{Queue, require_d1, require_queue};
use crate::envfile::EnvFile;
use crate::hf::{self, Hf};
use crate::names::{CRON, KOKORO_REPO, MING_REPO, QUEUE, SPACE_REPO, SPACE_URL, WORKER};
use crate::paths::{self, tilde};
use crate::secret::Secret;
use crate::state::{self, State};
use crate::timefmt;
use crate::worker_bundle::{Module, Vars, collect_modules, metadata};

pub struct Options {
    pub kokoro_rev: Option<String>,
    pub ming_rev: Option<String>,
}

#[derive(Deserialize)]
struct AccountSubdomain {
    subdomain: String,
}

pub fn run(opts: Options) -> Result<()> {
    // Everything that can fail locally is checked before the first request.
    let modules = collect_modules(&paths::worker_crate().join("build"))?;
    let hf_token = hf::load_token()?;
    let mut secrets = vec![("HF_TOKEN", hf_token.clone())];
    secrets.extend(load_login_secrets()?);
    let cf = Cf::from_deploy_env()?;
    let d1 = require_d1(&cf)?;
    let queue = require_queue(&cf)?;

    let hf = Hf::new(hf_token)?;
    let kokoro_rev = model_rev(&hf, KOKORO_REPO, opts.kokoro_rev)?;
    let ming_rev = model_rev(&hf, MING_REPO, opts.ming_rev)?;
    let vars = Vars {
        space_url: SPACE_URL.to_owned(),
        space_repo: SPACE_REPO.to_owned(),
        kokoro_rev,
        ming_rev,
        app_version: git_short_sha(),
    };

    upload_script(&cf, &d1.uuid, &vars, &modules)?;
    for (name, value) in &secrets {
        put_secret(&cf, name, value)?;
    }
    println!("secrets: {} set", secrets.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", "));
    ensure_consumer(&cf, &queue)?;
    let _: Value = cf.mutate(
        WORKER,
        Method::PUT,
        &cf.acct(&format!("/workers/scripts/{WORKER}/schedules")),
        Body::Json(json!([{ "cron": CRON }])),
    )?;
    println!("cron: {CRON}");
    let _: Value = cf.mutate(
        WORKER,
        Method::POST,
        &cf.acct(&format!("/workers/scripts/{WORKER}/subdomain")),
        Body::Json(json!({ "enabled": true, "previews_enabled": false })),
    )?;
    let sub: AccountSubdomain = cf.get(&cf.acct("/workers/subdomain"))?;
    let url = format!("https://{WORKER}.{}.workers.dev", sub.subdomain);

    let path = paths::state_file();
    let mut st = State::load(&path)?;
    st.d1 = Some(state::Named { name: d1.name, id: d1.uuid });
    st.queue = Some(state::Named { name: queue.queue_name, id: queue.queue_id });
    st.worker = Some(state::Worker {
        name: WORKER.to_owned(),
        url: url.clone(),
        app_version: vars.app_version,
        kokoro_rev: vars.kokoro_rev,
        ming_rev: vars.ming_rev,
        deployed_at: timefmt::now_rfc3339(),
    });
    st.save(&path)?;
    println!("{url}");
    Ok(())
}

/// The login secrets from app.env (`HF_TOKEN` comes from the Streams HF file).
fn load_login_secrets() -> Result<Vec<(&'static str, Secret)>> {
    let app_path = paths::app_env()?;
    if !app_path.exists() {
        bail!("{} not found: run `cargo xtask passphrase` first", tilde(&app_path));
    }
    let app = EnvFile::load(&app_path)?;
    Ok(vec![
        ("SESSION_KEY", app.secret("SESSION_KEY")?.clone()),
        ("PASSPHRASE_HASH", app.secret("PASSPHRASE_HASH")?.clone()),
    ])
}

/// `repo@sha`: the override if given (a bare sha or a full `repo@sha`), else the model's current `main` head, which
/// is what the Space's `preload_from_hub` downloads at build time.
fn model_rev(hf: &Hf, repo: &str, given: Option<String>) -> Result<String> {
    let sha = match given {
        Some(g) => g.rsplit_once('@').map_or(g.clone(), |(_, sha)| sha.to_owned()),
        None => hf.model_rev(repo).with_context(|| format!("cannot resolve {repo}; pass the revision explicitly"))?,
    };
    if sha.is_empty() || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("revision for {repo} must be a hex commit id");
    }
    Ok(format!("{repo}@{sha}"))
}

fn upload_script(cf: &Cf, d1_id: &str, vars: &Vars, modules: &[Module]) -> Result<()> {
    let meta = metadata(d1_id, vars);
    let mut form = Form::new().part("metadata", Part::text(meta.to_string()).mime_str("application/json")?);
    let mut total = 0usize;
    for m in modules {
        let bytes = fs::read(&m.path).with_context(|| format!("cannot read {}", tilde(&m.path)))?;
        total += bytes.len();
        form = form.part(m.name.clone(), Part::bytes(bytes).file_name(m.name.clone()).mime_str(m.content_type)?);
    }
    let path = cf.acct(&format!("/workers/scripts/{WORKER}"));
    let _: Value = cf.mutate(WORKER, Method::PUT, &path, Body::Multipart(form))?;
    println!("uploaded {WORKER}: {} module(s), {total} bytes, version {}", modules.len(), vars.app_version);
    Ok(())
}

fn put_secret(cf: &Cf, name: &str, value: &Secret) -> Result<()> {
    if value.is_empty() {
        bail!("secret {name} is empty");
    }
    let body = json!({ "name": name, "text": value.expose(), "type": "secret_text" });
    let path = cf.acct(&format!("/workers/scripts/{WORKER}/secrets"));
    let _: Value =
        cf.mutate(WORKER, Method::PUT, &path, Body::Json(body)).with_context(|| format!("setting {name}"))?;
    Ok(())
}

/// What to do about the queue's consumer list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsumerAction {
    Create,
    Update(String),
}

pub fn consumer_settings() -> Value {
    json!({ "batch_size": 1, "max_retries": 0, "max_wait_time_ms": 0 })
}

pub fn consumer_body() -> Value {
    json!({ "script_name": WORKER, "type": "worker", "settings": consumer_settings() })
}

/// Decides between creating and updating our consumer. A queue has one Worker consumer, so a consumer that is some
/// other script is an error rather than something to overwrite.
pub fn consumer_action(existing: &[Value]) -> Result<ConsumerAction> {
    let script = |c: &Value| {
        c.get("script").or_else(|| c.get("script_name")).and_then(Value::as_str).unwrap_or_default().to_owned()
    };
    if let Some(other) = existing.iter().map(script).find(|s| s != WORKER) {
        bail!("queue {QUEUE} is consumed by {other:?}, not {WORKER}");
    }
    match existing.first() {
        None => Ok(ConsumerAction::Create),
        Some(c) => {
            let id = c.get("consumer_id").and_then(Value::as_str).context("consumer without consumer_id")?;
            Ok(ConsumerAction::Update(id.to_owned()))
        }
    }
}

fn ensure_consumer(cf: &Cf, queue: &Queue) -> Result<()> {
    let base = cf.acct(&format!("/queues/{}/consumers", queue.queue_id));
    let existing: Vec<Value> = cf.get(&base)?;
    match consumer_action(&existing)? {
        ConsumerAction::Create => {
            let _: Value = cf.mutate(QUEUE, Method::POST, &base, Body::Json(consumer_body()))?;
            println!("queue {QUEUE}: consumer {WORKER} created");
        }
        ConsumerAction::Update(id) => {
            let _: Value = cf.mutate(QUEUE, Method::PUT, &format!("{base}/{id}"), Body::Json(consumer_body()))?;
            println!("queue {QUEUE}: consumer {WORKER} updated");
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn consumer_settings_match_note_09() {
        let b = consumer_body();
        assert_eq!(b["script_name"], "nolabs-api");
        assert_eq!(b["type"], "worker");
        assert_eq!(b["settings"], json!({"batch_size":1,"max_retries":0,"max_wait_time_ms":0}));
    }

    #[test]
    fn consumer_create_update_or_refuse() {
        assert_eq!(consumer_action(&[]).unwrap(), ConsumerAction::Create);
        let ours = json!({"consumer_id":"c1","script":"nolabs-api","settings":{"batch_size":10}});
        assert_eq!(consumer_action(&[ours]).unwrap(), ConsumerAction::Update("c1".into()));
        let legacy = json!({"consumer_id":"c2","script_name":"nolabs-api"});
        assert_eq!(consumer_action(&[legacy]).unwrap(), ConsumerAction::Update("c2".into()));
        let foreign = json!({"consumer_id":"c3","script":"streams-api"});
        assert!(consumer_action(&[foreign]).unwrap_err().to_string().contains("streams-api"));
        assert!(consumer_action(&[json!({"script":"nolabs-api"})]).is_err());
    }
}
