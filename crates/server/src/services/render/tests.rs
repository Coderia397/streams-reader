use super::*;
use crate::testing::{TestEnv, run};
use nolabs_core::proto::api::{Boundary, Emotion, Priority, RenderItem};
use nolabs_core::proto::engine::Mode;
use serde_json::json;

fn item(text: &str) -> RenderItem {
    RenderItem { text: text.into(), boundary: Boundary::Sentence, emotion: Emotion::Neutral }
}

fn request(voice: &str, texts: &[&str]) -> RenderRequest {
    RenderRequest {
        voice: voice.into(),
        items: texts.iter().map(|t| item(t)).collect(),
        priority: Priority::Interactive,
    }
}

fn job_request(env: &TestEnv, job_id: &str) -> EngineRequest {
    let job = run(env.app.repo().get_job(job_id)).unwrap().unwrap();
    serde_json::from_str(&job.request_json).unwrap()
}

#[test]
fn kokoro_claims_misses_dedupes_and_groups_with_a_ramp() {
    let env = TestEnv::new();
    let res = run(env.app.render(request("kokoro:af_heart", &["a", "b", "c", "a", "d", "e"]))).unwrap();
    assert_eq!(res.items.len(), 6);
    assert_eq!(res.items[0].key, res.items[3].key);
    assert!(res.items.iter().all(|s| s.state == RenderState::Queued));

    let sent = env.queue.take();
    assert_eq!(sent.len(), 3, "jobs of 1, 3 and 1 items");
    let sizes: Vec<usize> = sent.iter().map(|(m, _)| job_request(&env, &m.job_id).items.len()).collect();
    assert_eq!(sizes, vec![1, 3, 1]);
    let first = job_request(&env, &sent[0].0.job_id);
    assert_eq!(first.engine, Engine::Kokoro);
    assert_eq!(first.voice, EngineVoice::Preset { id: "af_heart".into() });
    assert_eq!(first.items[0].mode, Mode::Kokoro);
    assert_eq!(first.items[0].object_key, format!("audio/kokoro/{}.webm", first.items[0].key));
    assert_eq!(first.marker_key, format!("jobs/{}.json", first.job_id));
    assert_eq!(first.audio.id, 1);
    assert_eq!(env.db.scalar("SELECT count(*) FROM renders WHERE state = 'queued' AND job_id IS NOT NULL"), json!(5));

    // The same request again claims nothing.
    let again = run(env.app.render(request("kokoro:af_heart", &["a", "b"]))).unwrap();
    assert!(again.items.iter().all(|s| s.state == RenderState::Queued));
    assert!(env.queue.take().is_empty());
}

#[test]
fn kokoro_ignores_emotion_in_the_key() {
    let env = TestEnv::new();
    let mut req = request("kokoro:af_heart", &["same", "same"]);
    req.items[1].emotion = Emotion::Happy;
    let res = run(env.app.render(req)).unwrap();
    assert_eq!(res.items[0].key, res.items[1].key);
}

#[test]
fn ready_rows_are_returned_with_audio() {
    let env = TestEnv::new();
    let res = run(env.app.render(request("kokoro:bf_emma", &["hello"]))).unwrap();
    let key = &res.items[0].key;
    env.db.exec(&format!(
        "UPDATE renders SET state = 'ready', duration_ms = 1200, bytes = 4800, words_json = '[[0,500,0,5]]' \
         WHERE key = '{key}'"
    ));
    let res = run(env.app.render(request("kokoro:bf_emma", &["hello"]))).unwrap();
    assert_eq!(res.items[0].state, RenderState::Ready);
    assert_eq!(res.items[0].audio.as_ref().unwrap().bytes, 4800);
}

#[test]
fn failed_rows_are_reclaimed_only_after_their_retry_time() {
    let env = TestEnv::new();
    let res = run(env.app.render(request("kokoro:af_heart", &["x"]))).unwrap();
    env.queue.take();
    let key = &res.items[0].key;
    let retry_at = env.now() + 60_000;
    env.db.exec(&format!(
        "UPDATE renders SET state = 'failed', error_code = 'engine_failed', error_message = 'boom', \
         retry_at_ms = {retry_at} WHERE key = '{key}'"
    ));
    let res = run(env.app.render(request("kokoro:af_heart", &["x"]))).unwrap();
    assert_eq!(res.items[0].state, RenderState::Failed);
    assert_eq!(res.items[0].error.as_ref().unwrap().retry_after_s, Some(60));
    assert!(env.queue.take().is_empty());

    env.advance(60_000);
    let res = run(env.app.render(request("kokoro:af_heart", &["x"]))).unwrap();
    assert_eq!(res.items[0].state, RenderState::Queued);
    assert_eq!(env.queue.take().len(), 1);
}

#[test]
fn ming_preset_job_is_admitted_against_the_budget() {
    let env = TestEnv::new();
    let mut req = request("ming:US_Emma", &["One paragraph.", "Another one."]);
    req.items[1].emotion = Emotion::Sad;
    let res = run(env.app.render(req)).unwrap();
    assert!(res.items.iter().all(|s| s.state == RenderState::Queued));
    let sent = env.queue.take();
    assert_eq!(sent.len(), 1, "one GPU call for the whole request");
    let job = run(env.app.repo().get_job(&sent[0].0.job_id)).unwrap().unwrap();
    assert_eq!(job.requested_s, budget::estimate_seconds(&env.app.config.gpu, 26));
    let er: EngineRequest = serde_json::from_str(&job.request_json).unwrap();
    assert_eq!(er.engine, Engine::Ming);
    assert_eq!(er.audio.id, 2);
    assert_eq!((er.items[0].mode, er.items[1].mode), (Mode::Ip, Mode::Emotion));
    assert_eq!(er.items[1].emotion, Emotion::Sad);
}

#[test]
fn ming_without_budget_is_deferred_with_retry_after() {
    let env = TestEnv::new();
    // Three runs in the window: no runs left.
    let t = env.now() - 3_600_000;
    env.db.exec(&format!(
        "INSERT INTO gpu_ledger (job_id, attempt, at_ms, requested_s, used_s) VALUES \
         ('a', 1, {t}, 20, 20), ('b', 1, {t}, 20, 20), ('c', 1, {t}, 20, 20)"
    ));
    let res = run(env.app.render(request("ming:UK_George", &["Wait for it."]))).unwrap();
    let s = &res.items[0];
    assert_eq!(s.state, RenderState::Deferred);
    let e = s.error.as_ref().unwrap();
    assert_eq!(e.code, ErrorCode::GpuBudget);
    assert_eq!(e.retry_after_s, Some(23 * 3600));
    assert!(env.queue.take().is_empty());
    assert_eq!(env.db.scalar("SELECT state FROM jobs"), json!("deferred"));
    assert_eq!(env.db.scalar("SELECT retry_at_ms FROM jobs"), json!(t + DAY_MS));
    // The status route reports the same.
    let st = run(env.app.render_status(std::slice::from_ref(&s.key))).unwrap();
    assert_eq!(st.items[0].state, RenderState::Deferred);
}

#[test]
fn queued_ming_jobs_count_against_the_budget_before_they_run() {
    let env = TestEnv::new();
    // Each request is near the per-call maximum; the scheduler factor makes two of them exceed the daily cap.
    let long: Vec<String> = (0..3).map(|i| format!("{i}{}", "x".repeat(899))).collect();
    let texts: Vec<&str> = long.iter().map(String::as_str).collect();
    let first = run(env.app.render(request("ming:US_James", &texts))).unwrap();
    assert!(first.items.iter().all(|s| s.state == RenderState::Queued));
    assert_eq!(env.queue.take().len(), 1);
    let more: Vec<String> = (0..3).map(|i| format!("{i}{}", "y".repeat(899))).collect();
    let texts: Vec<&str> = more.iter().map(String::as_str).collect();
    let second = run(env.app.render(request("ming:US_James", &texts))).unwrap();
    assert!(second.items.iter().all(|s| s.state == RenderState::Deferred), "{second:?}");
}

#[test]
fn user_voices_must_be_ready_and_use_their_reference() {
    let env = TestEnv::new();
    let id = "0123456789abcdef";
    env.db.exec(&format!(
        "INSERT INTO voices (id, name, state, transcript, reference_bytes, created_at_ms, updated_at_ms) \
         VALUES ('{id}', 'Me', 'preparing', 'script', 10, 0, 0)"
    ));
    let e = run(env.app.render(request(&format!("user:{id}"), &["Hi."]))).unwrap_err();
    assert_eq!(e.code, ErrorCode::Conflict);
    env.db.exec(&format!(
        "UPDATE voices SET state = 'ready', ref_key = 'voices/{id}/ref.wav', ref_transcript = 'kept', \
         ref_sha256 = 'abc' WHERE id = '{id}'"
    ));
    let res = run(env.app.render(request(&format!("user:{id}"), &["Hi."]))).unwrap();
    let sent = env.queue.take();
    let er = job_request(&env, &sent[0].0.job_id);
    assert_eq!(
        er.voice,
        EngineVoice::Reference { id: id.into(), ref_key: format!("voices/{id}/ref.wav"), transcript: "kept".into() }
    );
    assert_eq!(er.items[0].mode, Mode::Tts);
    // Same text with a re-recorded reference gets a different key.
    env.db.exec(&format!("UPDATE voices SET ref_sha256 = 'def' WHERE id = '{id}'"));
    let res2 = run(env.app.render(request(&format!("user:{id}"), &["Hi."]))).unwrap();
    assert_ne!(res.items[0].key, res2.items[0].key);
    assert_eq!(env.db.scalar("SELECT voice FROM renders LIMIT 1"), json!(format!("user:{id}")));
}

#[test]
fn request_validation() {
    let env = TestEnv::new();
    let bad = |r: RenderRequest| run(env.app.render(r)).unwrap_err().code;
    assert_eq!(bad(request("kokoro:af_heart", &[])), ErrorCode::BadRequest);
    assert_eq!(bad(request("kokoro:af_heart", &["  "])), ErrorCode::BadRequest);
    assert_eq!(bad(request("kokoro:af_heart", &[&"x".repeat(MAX_ITEM_CHARS + 1)])), ErrorCode::BadRequest);
    assert_eq!(bad(request("kokoro:nobody", &["x"])), ErrorCode::BadRequest);
    assert_eq!(bad(request("user:0123456789abcdef", &["x"])), ErrorCode::NotFound);
    let many: Vec<&str> = vec!["x"; MAX_ITEMS + 1];
    assert_eq!(bad(request("kokoro:af_heart", &many)), ErrorCode::BadRequest);
}

#[test]
fn status_keeps_order_and_reports_unknown_keys() {
    let env = TestEnv::new();
    let res = run(env.app.render(request("kokoro:af_heart", &["one"]))).unwrap();
    let unknown = "f".repeat(64);
    let keys = vec![unknown.clone(), res.items[0].key.clone()];
    let st = run(env.app.render_status(&keys)).unwrap();
    assert_eq!(st.items[0].state, RenderState::Failed);
    assert_eq!(st.items[0].error.as_ref().unwrap().code, ErrorCode::NotFound);
    assert_eq!(st.items[1].state, RenderState::Queued);
}

#[test]
fn status_key_parsing() {
    let k = "a".repeat(64);
    assert_eq!(parse_status_keys(&format!("{k},{k}")).unwrap().len(), 2);
    assert!(parse_status_keys("").is_err());
    assert!(parse_status_keys("abc").is_err());
    let many = vec![k.as_str(); MAX_STATUS_KEYS + 1].join(",");
    assert!(parse_status_keys(&many).is_err());
}
