use super::*;
use crate::testing::fakes::{Reply, fake_space, response};
use crate::testing::{TestEnv, run};
use nolabs_core::proto::api::{Boundary, Emotion, Priority, RenderItem, RenderRequest, VoiceState};
use nolabs_core::proto::engine::{EngineItemError, PrepareReferenceResponse};
use serde_json::json;

fn render(env: &TestEnv, voice: &str, texts: &[&str]) -> Vec<String> {
    let req = RenderRequest {
        voice: voice.into(),
        items: texts
            .iter()
            .map(|t| RenderItem { text: (*t).into(), boundary: Boundary::Paragraph, emotion: Emotion::Neutral })
            .collect(),
        priority: Priority::Background,
    };
    run(env.app.render(&env.user("alice"), req)).unwrap();
    env.queue.take().into_iter().map(|(m, _)| m.job_id).collect()
}

/// An engine response for a request: every item ok, except keys whose text contains "FAIL".
fn engine_response(input: &str, gpu_ms: u64) -> EngineResponse {
    let req: EngineRequest = serde_json::from_str(input).unwrap();
    EngineResponse {
        v: 1,
        job_id: req.job_id,
        engine: req.engine,
        items: req
            .items
            .iter()
            .map(|i| {
                let fail = i.text.contains("FAIL");
                EngineItemResult {
                    key: i.key.clone(),
                    ok: !fail,
                    object_key: if fail { String::new() } else { i.object_key.clone() },
                    duration_ms: if fail { 0 } else { 2000 },
                    bytes: if fail { 0 } else { 8000 },
                    words: vec![[0, 400, 0, 3]],
                    error: fail.then(|| EngineItemError { code: "synthesis".into(), message: "empty audio".into() }),
                }
            })
            .collect(),
        elapsed_ms: 1000,
        gpu_ms,
    }
}

fn space_ok(env: &TestEnv, gpu_ms: u64) {
    fake_space(
        &env.http,
        move |_, input| Reply::Ok(serde_json::to_string(&engine_response(input, gpu_ms)).unwrap()),
        |r| panic!("unexpected request {}", r.url),
    );
}

fn space_fails(env: &TestEnv, reply: impl Fn() -> Reply + 'static) {
    fake_space(&env.http, move |_, _| reply(), |r| panic!("unexpected request {}", r.url));
}

fn handle(env: &TestEnv, job_id: &str) -> Outcome {
    run(env.app.handle_job(&JobMessage { job_id: job_id.into() })).unwrap()
}

fn states(env: &TestEnv) -> Vec<String> {
    let rows = run(env.app.db.all(&Stmt::new("SELECT state FROM renders ORDER BY chars, key", vec![]))).unwrap();
    rows.iter().map(|r| r["state"].as_str().unwrap().to_owned()).collect()
}

fn job(env: &TestEnv, id: &str) -> JobRow {
    run(env.app.repo().get_job(id)).unwrap().unwrap()
}

#[test]
fn kokoro_job_renders_every_item() {
    let env = TestEnv::new();
    let jobs = render(&env, "kokoro:af_heart", &["One.", "Two."]);
    assert_eq!(jobs.len(), 1);
    space_ok(&env, 0);
    assert_eq!(handle(&env, &jobs[0]), Outcome::Done);
    assert_eq!(states(&env), ["ready", "ready"]);
    assert_eq!(job(&env, &jobs[0]).state, RenderState::Ready);
    assert_eq!(env.http.urls()[0], "POST https://space.test/gradio_api/call/render_cpu");
    assert_eq!(env.db.scalar("SELECT bytes FROM renders LIMIT 1"), json!(8000));
    assert_eq!(env.db.scalar("SELECT words_json FROM renders LIMIT 1"), json!("[[0,400,0,3]]"));
    assert_eq!(env.db.scalar("SELECT count(*) FROM gpu_ledger"), json!(0));
    // A duplicate message for the finished job does nothing.
    assert_eq!(handle(&env, &jobs[0]), Outcome::Skipped { reason: "job is not queued" });
    assert_eq!(handle(&env, "nope"), Outcome::Skipped { reason: "unknown job" });
}

#[test]
fn partial_failure_marks_only_the_failed_items() {
    let env = TestEnv::new();
    let jobs = render(&env, "kokoro:af_heart", &["Fine.", "FAIL here."]);
    space_ok(&env, 0);
    assert_eq!(handle(&env, &jobs[0]), Outcome::PartlyFailed { failed: 1 });
    assert_eq!(states(&env), ["ready", "failed"]);
    assert_eq!(
        env.db.scalar("SELECT error_message FROM renders WHERE state = 'failed'"),
        json!("synthesis: empty audio")
    );
    assert_eq!(env.db.scalar("SELECT retry_at_ms FROM renders WHERE state = 'failed'"), json!(env.now() + 60_000));
}

#[test]
fn transient_kokoro_errors_requeue_with_backoff() {
    let env = TestEnv::new();
    let jobs = render(&env, "kokoro:af_heart", &["Hello."]);
    space_fails(&env, || Reply::Http(503, "<html>Space is sleeping</html>".into()));
    assert_eq!(handle(&env, &jobs[0]), Outcome::Retrying { delay_s: 30 });
    let j = job(&env, &jobs[0]);
    assert_eq!((j.state, j.attempts), (RenderState::Queued, 1));
    assert_eq!(states(&env), ["queued"]);
    let sent = env.queue.take();
    assert_eq!(sent, vec![(JobMessage { job_id: jobs[0].clone() }, 30)]);
    // The retry succeeds.
    env.advance(30_000);
    space_ok(&env, 0);
    assert_eq!(handle(&env, &jobs[0]), Outcome::Done);
    assert_eq!(job(&env, &jobs[0]).attempts, 2);
}

#[test]
fn bad_input_fails_the_job_and_its_items() {
    let env = TestEnv::new();
    let jobs = render(&env, "kokoro:af_heart", &["Hello."]);
    space_fails(&env, || Reply::Error("bad request: text too long".into()));
    assert!(matches!(handle(&env, &jobs[0]), Outcome::Failed { .. }));
    assert_eq!(states(&env), ["failed"]);
    assert_eq!(env.db.scalar("SELECT error_code FROM renders"), json!("bad_request"));
    assert!(env.queue.take().is_empty());
}

#[test]
fn ming_success_records_gpu_time_in_the_ledger() {
    let env = TestEnv::new();
    let jobs = render(&env, "ming:US_Emma", &["A paragraph.", "Another."]);
    let requested = job(&env, &jobs[0]).requested_s;
    space_ok(&env, 25_800);
    assert_eq!(handle(&env, &jobs[0]), Outcome::Done);
    assert_eq!(env.http.urls()[0], "POST https://space.test/gradio_api/call/render_gpu");
    assert_eq!(states(&env), ["ready", "ready"]);
    let row = run(env.app.db.all(&Stmt::new("SELECT * FROM gpu_ledger", vec![]))).unwrap();
    assert_eq!(row.len(), 1);
    assert_eq!((row[0]["requested_s"].clone(), row[0]["used_s"].clone()), (json!(requested), json!(26)));
}

#[test]
fn ming_marker_is_applied_without_calling_the_space() {
    let env = TestEnv::new();
    let jobs = render(&env, "ming:US_Emma", &["Marked."]);
    let request = job(&env, &jobs[0]).request_json;
    let marker = engine_response(&request, 9_000);
    env.blob.insert(&format!("jobs/{}.json", jobs[0]), serde_json::to_vec(&marker).unwrap());
    assert_eq!(handle(&env, &jobs[0]), Outcome::AppliedMarker);
    assert!(env.http.urls().is_empty());
    assert_eq!(states(&env), ["ready"]);
    assert_eq!(job(&env, &jobs[0]).state, RenderState::Ready);
}

#[test]
fn ming_quota_error_defers_job_and_items() {
    let env = TestEnv::new();
    let jobs = render(&env, "ming:US_Emma", &["Quota."]);
    space_fails(&env, || {
        Reply::Error("You have exceeded your GPU quota (60s requested vs. 10s left). Try again in 0:10:00".into())
    });
    let out = handle(&env, &jobs[0]);
    assert_eq!(out, Outcome::Deferred { retry_at_ms: env.now() + 600_000 });
    assert_eq!(states(&env), ["deferred"]);
    let st = run(env.app.render_status(&[env.db.scalar("SELECT key FROM renders").as_str().unwrap().to_owned()]));
    assert_eq!(st.unwrap().items[0].error.as_ref().unwrap().retry_after_s, Some(600));
    assert!(env.queue.take().is_empty(), "GPU jobs are never re-sent by the consumer");
    // ZeroGPU refused the call before it ran: no ledger entry is left behind.
    assert_eq!(env.db.scalar("SELECT count(*) FROM gpu_ledger"), json!(0));
}

#[test]
fn ming_refused_by_a_sleeping_space_fails_briefly_and_costs_no_budget() {
    let env = TestEnv::new();
    let jobs = render(&env, "ming:US_Emma", &["Asleep."]);
    space_fails(&env, || Reply::Http(503, "<!doctype html><title>Space sleeping</title>".into()));
    assert!(matches!(handle(&env, &jobs[0]), Outcome::Failed { .. }));
    assert_eq!(states(&env), ["failed"]);
    assert_eq!(env.db.scalar("SELECT retry_at_ms FROM renders"), json!(env.now() + 60_000));
    assert_eq!(env.db.scalar("SELECT count(*) FROM gpu_ledger"), json!(0));
}

#[test]
fn ming_internal_error_fails_without_retry() {
    let env = TestEnv::new();
    let jobs = render(&env, "ming:US_Emma", &["Boom."]);
    space_fails(&env, || Reply::Error("engine error: RuntimeError: CUDA error".into()));
    assert!(matches!(handle(&env, &jobs[0]), Outcome::Failed { .. }));
    assert_eq!(states(&env), ["failed"]);
    assert_eq!(env.db.scalar("SELECT retry_at_ms FROM renders"), json!(env.now() + 3_600_000));
    assert!(env.queue.take().is_empty());
    // The GPU may have run: the attempt stays in the ledger.
    assert_eq!(env.db.scalar("SELECT count(*) FROM gpu_ledger"), json!(1));
}

#[test]
fn ming_budget_is_checked_again_before_the_call() {
    let env = TestEnv::new();
    let jobs = render(&env, "ming:US_Emma", &["Later."]);
    let t = env.now();
    env.db.exec(&format!(
        "INSERT INTO gpu_ledger (job_id, attempt, at_ms, requested_s, used_s) VALUES \
         ('a', 1, {t}, 10, 10), ('b', 1, {t}, 10, 10), ('c', 1, {t}, 10, 10)"
    ));
    space_ok(&env, 0);
    assert!(matches!(handle(&env, &jobs[0]), Outcome::Deferred { .. }));
    assert!(env.http.urls().is_empty());
    assert_eq!(job(&env, &jobs[0]).state, RenderState::Deferred);
    assert_eq!(states(&env), ["deferred"]);
}

fn prepared_voice(env: &TestEnv) -> (String, String) {
    let id = "0123456789abcdef".to_owned();
    env.db.exec(&format!(
        "INSERT INTO voices (id, user_id, name, state, transcript, reference_bytes, raw_uploaded, created_at_ms, \
         updated_at_ms) VALUES ('{id}', 'alice', 'Me', 'awaiting_reference', 'line one\nline two', 10, 1, 0, 0)"
    ));
    env.accounts.add_voice("alice", &id);
    run(env.app.prepare_voice(&env.user("alice"), &id)).unwrap();
    let job_id = env.queue.take()[0].0.job_id.clone();
    (id, job_id)
}

#[test]
fn prepare_job_makes_the_voice_ready() {
    let env = TestEnv::new();
    let (id, job_id) = prepared_voice(&env);
    let reply = PrepareReferenceResponse {
        v: 1,
        ref_key: format!("voices/{id}/ref.wav"),
        transcript: "line two".into(),
        duration_ms: 12_000,
        sha256: "ab".repeat(32),
    };
    let body = serde_json::to_string(&reply).unwrap();
    fake_space(
        &env.http,
        move |api, _| {
            assert_eq!(api, "prepare_reference");
            Reply::Ok(body.clone())
        },
        |_| Ok(response(500, &[], "")),
    );
    assert_eq!(handle(&env, &job_id), Outcome::Done);
    let v = run(env.app.repo().get_voice(&id, "alice")).unwrap().unwrap();
    assert_eq!(v.state, VoiceState::Ready);
    assert_eq!(v.ref_transcript.as_deref(), Some("line two"));
    assert_eq!(v.ref_sha256, Some("ab".repeat(32)));
    assert_eq!(job(&env, &job_id).state, RenderState::Ready);
}

#[test]
fn prepare_rejected_by_the_space_fails_the_voice() {
    let env = TestEnv::new();
    let (id, job_id) = prepared_voice(&env);
    space_fails(&env, || Reply::Error("bad request: recording is silent".into()));
    assert!(matches!(handle(&env, &job_id), Outcome::Failed { .. }));
    let v = run(env.app.repo().get_voice(&id, "alice")).unwrap().unwrap();
    assert_eq!(v.state, VoiceState::Failed);
    assert!(v.error.unwrap().contains("silent"));
}
