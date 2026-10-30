//! `smoke`: drives the deployed system the way the app does, as a throwaway user ([`crate::test_user`]): check the
//! session, create a two-paragraph paste document in Supabase, upload its body through the Worker, read it back,
//! render it with Kokoro, wait for the audio, fetch the first clip with the access token and again with an audio-URL
//! token, read `/api/status`, then delete the document. Each run's text carries a run number so the render is never
//! a cache hit.

use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use nolabs_core::cache_key::is_key;
use nolabs_core::ids;
use nolabs_core::proto::api::{
    AUDIO_TOKEN_PARAM, AudioToken, Block, BlockKind, DOCUMENT_BODY_VERSION, DocumentBody, DocumentMeta, DocumentRecord,
    Emotion, Priority, RenderItem, RenderRequest, RenderResponse, RenderState, SessionInfo, SourceKind, SystemStatus,
};
use nolabs_core::text::{self, Segment, SegmentProfile};
use nolabs_core::voices::DEFAULT_VOICE;
use reqwest::Method;
use serde_json::Value;

use crate::api::{Api, Fetched};
use crate::cmd::status::{print_system_status, resolve_base};
use crate::test_user::{TestUser, with_test_user};
use crate::timefmt;

const RENDER_DEADLINE: Duration = Duration::from_secs(180);
const EBML_MAGIC: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];
const MIN_AUDIO_BYTES: usize = 1024;

/// Step timings, printed at the end whether the run passed or not.
#[derive(Default)]
struct Timings(Vec<(String, Duration, bool)>);

impl Timings {
    fn step<T>(&mut self, name: &str, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let t = Instant::now();
        let r = f();
        self.0.push((name.to_owned(), t.elapsed(), r.is_ok()));
        r
    }

    fn print(&self) {
        println!("\n{:<28} {:>9}", "step", "seconds");
        for (name, d, ok) in &self.0 {
            println!("{name:<28} {:>9.2}{}", d.as_secs_f64(), if *ok { "" } else { "  FAILED" });
        }
        let total: Duration = self.0.iter().map(|(_, d, _)| *d).sum();
        println!("{:<28} {:>9.2}", "total", total.as_secs_f64());
    }
}

pub fn run(base: Option<String>) -> Result<()> {
    let base = resolve_base(base)?;
    with_test_user("smoke", |user| {
        let api = Api::new(&base, &user.session.access_token)?;
        let mut t = Timings::default();
        println!("smoke test against {}", api.base());
        let result = steps(&api, user, &mut t);
        t.print();
        result
    })?;
    println!("smoke test passed");
    Ok(())
}

fn steps(api: &Api, user: &TestUser, t: &mut Timings) -> Result<()> {
    let session: SessionInfo = t.step("GET /api/session", || api.get_json("/api/session"))?;
    if session.user_id != user.session.user_id {
        bail!("the Worker sees user {}, Supabase signed in {}", session.user_id, user.session.user_id);
    }
    println!("session for {} expires at {} (unix ms)", user.email, session.expires_at_ms);

    let blocks = text::normalise_blocks(smoke_blocks(timefmt::now_unix()));
    let (record, body) = document_upload(new_id()?, &blocks)?;
    let row = serde_json::to_value(&record)?;
    t.step("insert row (PostgREST)", || {
        user.project.rest(&user.session, Method::POST, "documents", Some(&row)).map(drop)
    })?;
    let doc_path = format!("/api/documents/{}", record.id);
    let exercised = exercise(api, user, t, &record.id, body, &blocks);
    let deleted = t.step("DELETE document", || api.delete(&doc_path));
    let gone = rows(user, &record.id).and_then(|r| if r.is_empty() { Ok(()) } else { bail!("row still exists") });
    exercised.and(deleted).and(gone)
}

/// The caller's view of one document row through PostgREST.
fn rows(user: &TestUser, id: &str) -> Result<Vec<Value>> {
    let v = user.project.rest(&user.session, Method::GET, &format!("documents?select=ready&id=eq.{id}"), None)?;
    Ok(v.as_array().cloned().unwrap_or_default())
}

fn new_id() -> Result<String> {
    let mut bytes = [0u8; 10];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("no OS randomness: {e}"))?;
    Ok(ids::id_from_bytes(bytes))
}

fn exercise(api: &Api, user: &TestUser, t: &mut Timings, id: &str, body: Vec<u8>, blocks: &[Block]) -> Result<()> {
    let doc_path = format!("/api/documents/{id}");
    t.step("PUT body", || api.put_bytes(&format!("{doc_path}/body"), "application/json", body.clone()))?;
    if rows(user, id)?.first().and_then(|r| r.get("ready")).and_then(Value::as_bool) != Some(true) {
        bail!("the Worker did not mark the document ready in Supabase");
    }
    let back = t.step("GET body", || api.get_bytes(&format!("{doc_path}/body")))?;
    if back.bytes != body {
        bail!("the body read back differs from the upload");
    }

    let segments = text::segment(blocks, SegmentProfile::KOKORO);
    let request = render_request(&segments);
    let rendered: RenderResponse = t.step("POST /api/render", || api.post_json("/api/render", &request))?;
    if rendered.items.len() != segments.len() {
        bail!("asked for {} segments, got {} items", segments.len(), rendered.items.len());
    }
    let keys: Vec<String> = rendered.items.iter().map(|i| i.key.clone()).collect();
    println!("rendering {} segment(s) with {DEFAULT_VOICE}", keys.len());
    t.step("wait for audio", || wait_ready(api, &keys))?;

    let first = keys.first().context("no segments")?;
    let audio = t.step("GET /api/audio (first)", || api.get_bytes(&format!("/api/audio/{first}")))?;
    check_webm(&audio)?;
    println!("first clip: {} bytes, {}", audio.bytes.len(), audio.content_type);
    let token: AudioToken =
        t.step("POST /api/audio-token", || api.post_json("/api/audio-token", &serde_json::json!({})))?;
    let signed = format!("/api/audio/{first}?{AUDIO_TOKEN_PARAM}={}", token.token);
    let via_token = t.step("GET /api/audio?t= (no header)", || api.get_bytes_unauthenticated(&signed))?;
    if via_token.bytes != audio.bytes {
        bail!("audio fetched with the audio token differs");
    }

    let status: SystemStatus = t.step("GET /api/status", || api.get_json("/api/status"))?;
    print_system_status(&status);
    Ok(())
}

fn wait_ready(api: &Api, keys: &[String]) -> Result<()> {
    let path = status_path(keys)?;
    let started = Instant::now();
    let mut backoff = Backoff::new();
    loop {
        let status: RenderResponse = api.get_json(&path)?;
        let ready = status.items.iter().filter(|i| i.state == RenderState::Ready).count();
        if let Some(failed) = status.items.iter().find(|i| i.state == RenderState::Failed) {
            let why = failed.error.as_ref().map_or_else(|| "no detail".to_owned(), ToString::to_string);
            bail!("segment {} failed: {why}", failed.key);
        }
        if ready == keys.len() && status.items.len() == keys.len() {
            return Ok(());
        }
        let Some(delay) = backoff.next_delay(started.elapsed(), RENDER_DEADLINE) else {
            bail!("only {ready}/{} segments ready after {} s", keys.len(), RENDER_DEADLINE.as_secs());
        };
        sleep(delay);
    }
}

/// Fixed text plus a run number, so every run renders instead of hitting the cache.
pub fn smoke_blocks(run: i64) -> Vec<Block> {
    let p = |text: String| Block { kind: BlockKind::Paragraph, text };
    vec![
        p(format!(
            "This is the nolabs smoke test, run number {run}. It checks that a short document can be imported, \
             rendered by the speech engine, stored, and played back through the deployed API."
        )),
        p("If you can hear this second paragraph, the second segment rendered as well. The test deletes this \
           document again when it finishes, so the library stays clean."
            .to_owned()),
    ]
}

/// The Supabase row and the exact body bytes it announces.
pub fn document_upload(id: String, blocks: &[Block]) -> Result<(DocumentRecord, Vec<u8>)> {
    let body = serde_json::to_vec(&DocumentBody { v: DOCUMENT_BODY_VERSION, blocks: blocks.to_vec() })?;
    let reading = text::reading_text(blocks);
    let meta = DocumentMeta {
        title: "nolabs smoke test".to_owned(),
        author: None,
        source: SourceKind::Paste,
        source_url: None,
        language: "en".to_owned(),
        char_count: reading.chars().count() as u64,
        word_count: text::word_count(&reading) as u64,
        block_count: u32::try_from(blocks.len())?,
    };
    Ok((DocumentRecord::new(id, meta, body.len() as u64), body))
}

pub fn render_request(segments: &[Segment]) -> RenderRequest {
    RenderRequest {
        voice: DEFAULT_VOICE.to_owned(),
        items: segments
            .iter()
            .map(|s| RenderItem { text: s.text.clone(), boundary: s.boundary, emotion: Emotion::Neutral })
            .collect(),
        priority: Priority::Interactive,
    }
}

/// `/api/render/status?keys=…`. Keys are validated first, so nothing needs URL-encoding.
pub fn status_path(keys: &[String]) -> Result<String> {
    if let Some(bad) = keys.iter().find(|k| !is_key(k)) {
        bail!("server returned a malformed cache key {bad:?}");
    }
    Ok(format!("/api/render/status?keys={}", keys.join(",")))
}

/// The first clip must be WebM (EBML magic), served as `audio/webm`, and more than a header.
pub fn check_webm(f: &Fetched) -> Result<()> {
    if !f.content_type.starts_with("audio/webm") {
        bail!("audio Content-Type is {:?}, expected audio/webm", f.content_type);
    }
    if f.bytes.len() <= MIN_AUDIO_BYTES {
        bail!("audio is only {} bytes", f.bytes.len());
    }
    if f.bytes[..4] != EBML_MAGIC {
        bail!("audio does not start with the EBML magic 1A 45 DF A3");
    }
    Ok(())
}

/// Poll delays: 1 s growing by half each time, capped at 10 s, never sleeping past the deadline.
#[derive(Debug, Clone)]
pub struct Backoff {
    next: Duration,
}

impl Backoff {
    const START: Duration = Duration::from_secs(1);
    const CAP: Duration = Duration::from_secs(10);

    pub fn new() -> Self {
        Self { next: Self::START }
    }

    /// The next sleep, or `None` once `elapsed` has reached `deadline`.
    pub fn next_delay(&mut self, elapsed: Duration, deadline: Duration) -> Option<Duration> {
        let left = deadline.checked_sub(elapsed).filter(|d| !d.is_zero())?;
        let delay = self.next.min(left);
        self.next = (self.next * 3 / 2).min(Self::CAP);
        Some(delay)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use nolabs_core::proto::api::Boundary;

    #[test]
    fn document_upload_announces_exact_body_length() {
        let blocks = smoke_blocks(42);
        let (record, body) = document_upload("0123456789abcdef".into(), &blocks).unwrap();
        assert_eq!(record.body_bytes, body.len() as u64);
        assert_eq!(record.block_count, 2);
        assert_eq!(record.source, SourceKind::Paste);
        assert!(!record.ready && record.user_id.is_none(), "the database fills user_id");
        assert_eq!(record.char_count, text::reading_text(&blocks).chars().count() as u64);
        let row = serde_json::to_value(&record).unwrap();
        assert!(row.get("user_id").is_none() && row.get("created_at").is_none());
        let parsed: DocumentBody = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed.v, DOCUMENT_BODY_VERSION);
        assert_eq!(parsed.blocks, blocks);
        assert!(blocks[0].text.contains("run number 42"));
    }

    #[test]
    fn smoke_paragraphs_fit_kokoro_limits() {
        for b in smoke_blocks(1_793_388_442) {
            let n = b.text.chars().count() as u32;
            let k = SegmentProfile::KOKORO;
            assert!((k.min_chars..=k.max_chars).contains(&n), "{n}");
        }
    }

    #[test]
    fn render_request_uses_default_voice_and_segment_text() {
        let seg = Segment {
            idx: 0,
            block: 0,
            text: "Hello there.".into(),
            boundary: Boundary::Paragraph,
            char_start: 0,
            char_end: 12,
            heading: false,
        };
        let r = render_request(&[seg]);
        assert_eq!(r.voice, "kokoro:af_heart");
        assert_eq!(r.items.len(), 1);
        assert_eq!(r.items[0].text, "Hello there.");
        assert_eq!(r.items[0].boundary, Boundary::Paragraph);
        assert_eq!(r.priority, Priority::Interactive);
    }

    #[test]
    fn status_path_validates_keys() {
        let k = "a".repeat(64);
        let path = status_path(&[k.clone(), k.clone()]);
        // Accept whatever is_key accepts; reject obvious junk.
        if is_key(&k) {
            assert_eq!(path.unwrap(), format!("/api/render/status?keys={k},{k}"));
        }
        assert!(status_path(&["../x".into()]).is_err());
        assert!(status_path(&["a,b".into()]).is_err());
    }

    #[test]
    fn webm_checks() {
        let mut bytes = EBML_MAGIC.to_vec();
        bytes.resize(2048, 0);
        let ok = Fetched { content_type: "audio/webm; codecs=\"opus\"".into(), bytes: bytes.clone() };
        check_webm(&ok).unwrap();
        let wrong_type = Fetched { content_type: "audio/ogg".into(), bytes: bytes.clone() };
        assert!(check_webm(&wrong_type).is_err());
        let short = Fetched { content_type: "audio/webm".into(), bytes: EBML_MAGIC.to_vec() };
        assert!(check_webm(&short).is_err());
        let mut ogg = bytes;
        ogg[..4].copy_from_slice(b"OggS");
        assert!(check_webm(&Fetched { content_type: "audio/webm".into(), bytes: ogg }).is_err());
    }

    #[test]
    fn backoff_grows_caps_and_stops_at_deadline() {
        let mut b = Backoff::new();
        let deadline = Duration::from_secs(180);
        let delays: Vec<u64> =
            (0..8).map(|_| b.next_delay(Duration::ZERO, deadline).unwrap().as_millis() as u64).collect();
        assert_eq!(delays, [1000, 1500, 2250, 3375, 5062, 7593, 10000, 10000]);
        assert_eq!(b.next_delay(Duration::from_secs(179), deadline), Some(Duration::from_secs(1)));
        assert_eq!(b.next_delay(deadline, deadline), None);
        assert_eq!(b.next_delay(Duration::from_secs(500), deadline), None);
    }
}
