use super::*;
use crate::ports::{HttpRequest, Method};
use crate::testing::fakes::{Reply, fake_space, response};
use crate::testing::{TestEnv, run};
use nolabs_core::proto::api::{Boundary, Emotion, Priority, RenderItem, RenderRequest};
use nolabs_core::proto::engine::{EngineItemResult, EngineResponse};
use serde_json::json;

const HEALTHY: &str = r#"{"v":1,"engines":{"kokoro":"ready","ming":"ready"},"storage_ok":true}"#;

/// A Space whose runtime API reports `stage`; health answers `health`; every hub call is recorded.
fn space(env: &TestEnv, stage: &'static str, health: &'static str) {
    fake_space(
        &env.http,
        move |api, _| match api {
            "health" => Reply::Ok(health.to_owned()),
            other => panic!("unexpected endpoint {other}"),
        },
        move |req: &HttpRequest| {
            assert_eq!(crate::ports::header_value(&req.headers, "authorization"), Some("Bearer hf_test"));
            Ok(match (req.method, req.url.as_str()) {
                (Method::Get, "https://huggingface.co/api/spaces/owner/space/runtime") => {
                    response(200, &[], format!(r#"{{"stage":"{stage}","hardware":{{"current":"zero-a10g"}}}}"#))
                }
                (Method::Post, "https://huggingface.co/api/spaces/owner/space/restart") => response(200, &[], "{}"),
                (_, other) => panic!("unexpected {other}"),
            })
        },
    );
}

fn render(env: &TestEnv, voice: &str, text: &str) -> String {
    let req = RenderRequest {
        voice: voice.into(),
        items: vec![RenderItem { text: text.into(), boundary: Boundary::Sentence, emotion: Emotion::Neutral }],
        priority: Priority::Background,
    };
    run(env.app.render(req)).unwrap();
    env.db.scalar("SELECT id FROM jobs ORDER BY created_at_ms DESC, rowid DESC LIMIT 1").as_str().unwrap().to_owned()
}

fn job_state(env: &TestEnv, id: &str) -> String {
    env.db.scalar(&format!("SELECT state FROM jobs WHERE id = '{id}'")).as_str().unwrap().to_owned()
}

fn item_state(env: &TestEnv, job: &str) -> String {
    env.db.scalar(&format!("SELECT state FROM renders WHERE job_id = '{job}'")).as_str().unwrap().to_owned()
}

#[test]
fn a_sleeping_space_is_restarted_and_its_stage_stored() {
    let env = TestEnv::new();
    space(&env, "SLEEPING", HEALTHY);
    let report = run(env.app.run_cron());
    assert_eq!(report.stage.as_deref(), Some("SLEEPING"));
    assert!(report.restarted);
    assert!(!report.healthy);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert!(env.http.urls().contains(&"POST https://huggingface.co/api/spaces/owner/space/restart".to_owned()));
    assert!(!env.http.urls().iter().any(|u| u.contains("gradio_api")));
    let status = run(env.app.space_status()).unwrap();
    assert_eq!((status.stage.as_str(), status.healthy, status.checked_at_ms), ("SLEEPING", false, env.now()));
}

#[test]
fn a_running_space_gets_a_health_call_and_no_restart() {
    let env = TestEnv::new();
    space(&env, "RUNNING", HEALTHY);
    let report = run(env.app.run_cron());
    assert!(!report.restarted);
    assert!(report.healthy);
    assert!(env.http.urls().contains(&"POST https://space.test/gradio_api/call/health".to_owned()));
    assert!(run(env.app.space_status()).unwrap().healthy);

    let env = TestEnv::new();
    space(&env, "RUNNING", r#"{"v":1,"engines":{"kokoro":"ready","ming":"failed: OOM"},"storage_ok":true}"#);
    assert!(!run(env.app.run_cron()).healthy);
}

#[test]
fn building_spaces_are_left_alone() {
    let env = TestEnv::new();
    space(&env, "BUILDING", HEALTHY);
    let report = run(env.app.run_cron());
    assert!(!report.restarted);
}

#[test]
fn expired_cpu_leases_are_requeued_and_gpu_leases_failed() {
    let env = TestEnv::new();
    space(&env, "RUNNING", HEALTHY);
    let cpu = render(&env, "kokoro:af_heart", "cpu");
    let gpu = render(&env, "ming:US_Emma", "gpu");
    env.queue.take();
    let past = env.now() - 1;
    env.db.exec(&format!(
        "UPDATE jobs SET state = 'rendering', attempts = 1, lease_until_ms = {past}; \
         UPDATE renders SET state = 'rendering'"
    ));
    let report = run(env.app.run_cron());
    assert_eq!(report.recovered, 2);
    assert_eq!((job_state(&env, &cpu), item_state(&env, &cpu)), ("queued".into(), "queued".into()));
    assert_eq!((job_state(&env, &gpu), item_state(&env, &gpu)), ("failed".into(), "failed".into()));
    assert_eq!(env.queue.take().into_iter().map(|(m, _)| m.job_id).collect::<Vec<_>>(), vec![cpu]);
}

fn marker_for(env: &TestEnv, job_id: &str) {
    let req: EngineRequest = serde_json::from_str(
        env.db.scalar(&format!("SELECT request_json FROM jobs WHERE id = '{job_id}'")).as_str().unwrap(),
    )
    .unwrap();
    let resp = EngineResponse {
        v: 1,
        job_id: job_id.into(),
        engine: Engine::Ming,
        items: req
            .items
            .iter()
            .map(|i| EngineItemResult {
                key: i.key.clone(),
                ok: true,
                object_key: i.object_key.clone(),
                duration_ms: 1000,
                bytes: 100,
                words: vec![],
                error: None,
            })
            .collect(),
        elapsed_ms: 1,
        gpu_ms: 3000,
    };
    env.blob.insert(&format!("jobs/{job_id}.json"), serde_json::to_vec(&resp).unwrap());
}

#[test]
fn gpu_jobs_with_a_marker_are_completed_instead_of_failed() {
    let env = TestEnv::new();
    space(&env, "RUNNING", HEALTHY);
    let expired = render(&env, "ming:US_Emma", "expired lease");
    let failed = render(&env, "ming:UK_George", "failed earlier");
    let past = env.now() - 1;
    env.db.exec(&format!(
        "UPDATE jobs SET state = 'rendering', attempts = 1, lease_until_ms = {past} WHERE id = '{expired}'; \
         UPDATE jobs SET state = 'failed', attempts = 1 WHERE id = '{failed}'; \
         UPDATE renders SET state = 'failed' WHERE job_id = '{failed}'"
    ));
    marker_for(&env, &expired);
    marker_for(&env, &failed);
    let report = run(env.app.run_cron());
    assert_eq!(report.markers_applied, 2);
    for job in [&expired, &failed] {
        assert_eq!((job_state(&env, job), item_state(&env, job)), ("ready".into(), "ready".into()));
    }
}

#[test]
fn due_deferred_gpu_jobs_are_readmitted_when_the_budget_allows() {
    let env = TestEnv::new();
    space(&env, "RUNNING", HEALTHY);
    let t = env.now() - 3_600_000;
    env.db.exec(&format!(
        "INSERT INTO gpu_ledger (job_id, attempt, at_ms, requested_s, used_s) VALUES \
         ('a', 1, {t}, 10, 10), ('b', 1, {t}, 10, 10), ('c', 1, {t}, 10, 10)"
    ));
    let job = render(&env, "ming:US_Emma", "waiting");
    assert_eq!(job_state(&env, &job), "deferred");
    // Not due yet: nothing happens.
    assert_eq!(run(env.app.run_cron()).released, 0);
    // Due, but the ledger still blocks it (pretend the window did not reset): stays deferred with a new time.
    env.db.exec(&format!("UPDATE jobs SET retry_at_ms = {} WHERE id = '{job}'", env.now()));
    let report = run(env.app.run_cron());
    assert_eq!((report.released, report.still_deferred), (0, 1));
    assert_eq!(job_state(&env, &job), "deferred");
    assert!(env.queue.take().is_empty());
    // The window resets: the job is admitted and sent.
    env.advance(23 * 3_600_000 + 1);
    let report = run(env.app.run_cron());
    assert_eq!(report.released, 1);
    assert_eq!((job_state(&env, &job), item_state(&env, &job)), ("queued".into(), "queued".into()));
    assert_eq!(env.queue.take().len(), 1);
}

#[test]
fn stale_queued_jobs_are_resent_and_orphans_released() {
    let env = TestEnv::new();
    space(&env, "RUNNING", HEALTHY);
    let job = render(&env, "kokoro:af_heart", "stale");
    env.queue.take();
    let now = env.now();
    env.db.exec(&format!(
        "INSERT INTO renders (key, engine, voice, state, object_key, chars, created_at_ms, updated_at_ms) \
         VALUES ('orphan', 'kokoro', 'kokoro:af_heart', 'queued', 'o', 1, {now}, {now})"
    ));
    let first = run(env.app.run_cron());
    assert_eq!((first.resent, first.orphans), (0, 0));
    env.advance(STALE_QUEUED_MS + 1);
    let report = run(env.app.run_cron());
    assert_eq!(report.resent, 1);
    assert_eq!(report.orphans, 1);
    assert_eq!(env.queue.take()[0].0.job_id, job);
    assert_eq!(env.db.scalar("SELECT state FROM renders WHERE key = 'orphan'"), json!("failed"));
    assert_eq!(env.db.scalar("SELECT retry_at_ms FROM renders WHERE key = 'orphan'"), json!(null));
    // Touched: not re-sent again on the next run.
    assert_eq!(run(env.app.run_cron()).resent, 0);
}

#[test]
fn a_failing_runtime_api_does_not_stop_the_other_steps() {
    let env = TestEnv::new();
    env.http.set(|_| Ok(response(500, &[], "oops")));
    let job = render(&env, "kokoro:af_heart", "x");
    env.queue.take();
    env.advance(STALE_QUEUED_MS + 1);
    let report = run(env.app.run_cron());
    assert!(report.errors.iter().any(|e| e.starts_with("runtime")));
    assert_eq!(report.stage.as_deref(), Some("UNKNOWN"));
    assert_eq!(report.resent, 1);
    assert_eq!(env.queue.take()[0].0.job_id, job);
}
