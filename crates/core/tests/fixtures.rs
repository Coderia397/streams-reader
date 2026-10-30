//! The shared contract fixtures in `contracts/fixtures/` must deserialise into the engine contract types and serialise
//! back to the same JSON value, so a field added, renamed or dropped on either side fails here and in the Space's
//! pytest suite. Fixtures are embedded at compile time; the directory listing check reads the folder at test time.

use std::collections::BTreeSet;

use nolabs_core::cache_key::{audio_object_key, is_key};
use nolabs_core::ids::is_id;
use nolabs_core::proto::api::Engine;
use nolabs_core::proto::engine::{
    CONTRACT_VERSION, EngineHealth, EngineRequest, EngineResponse, EngineVoice, Mode, PrepareReferenceRequest,
    PrepareReferenceResponse,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

const KOKORO_REQUEST: &str = include_str!("../../../contracts/fixtures/engine_request_kokoro.json");
const MING_REQUEST: &str = include_str!("../../../contracts/fixtures/engine_request_ming.json");
const RESPONSE: &str = include_str!("../../../contracts/fixtures/engine_response.json");
const PREPARE_REQUEST: &str = include_str!("../../../contracts/fixtures/prepare_reference_request.json");
const PREPARE_RESPONSE: &str = include_str!("../../../contracts/fixtures/prepare_reference_response.json");
const HEALTH: &str = include_str!("../../../contracts/fixtures/health.json");

/// Every fixture this test checks; a new file in the folder must be added here.
const CHECKED: [&str; 6] = [
    "engine_request_kokoro.json",
    "engine_request_ming.json",
    "engine_response.json",
    "health.json",
    "prepare_reference_request.json",
    "prepare_reference_response.json",
];

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Parse `json` as `T`, serialise it again and require the same JSON value (unknown or dropped fields fail).
#[track_caller]
fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
    json: &str,
) -> Result<T, Box<dyn std::error::Error>> {
    let original: Value = serde_json::from_str(json)?;
    let typed: T = serde_json::from_str(json)?;
    let again = serde_json::to_value(&typed)?;
    assert_eq!(again, original, "round trip changed the JSON");
    let reparsed: T = serde_json::from_value(again)?;
    assert_eq!(reparsed, typed);
    Ok(typed)
}

#[track_caller]
fn check_request(req: &EngineRequest, engine: Engine) {
    assert_eq!(req.v, CONTRACT_VERSION);
    assert_eq!(req.engine, engine);
    assert!(is_id(&req.job_id), "job_id {}", req.job_id);
    assert_eq!(req.marker_key, format!("jobs/{}.json", req.job_id));
    assert!(!req.items.is_empty());
    for item in &req.items {
        assert!(is_key(&item.key), "key {}", item.key);
        assert_eq!(item.object_key, audio_object_key(engine, &item.key));
        assert_eq!(item.text.trim(), item.text);
        assert_eq!(item.mode == Mode::Kokoro, engine == Engine::Kokoro, "mode {:?} on {engine:?}", item.mode);
    }
}

#[test]
fn kokoro_request_round_trips() -> TestResult {
    let req: EngineRequest = round_trip(KOKORO_REQUEST)?;
    check_request(&req, Engine::Kokoro);
    assert_eq!(req.voice, EngineVoice::Preset { id: "af_heart".into() });
    assert_eq!((req.audio.id, req.audio.bitrate_kbps, req.audio.sample_rate), (1, 32, 24_000));
    assert_eq!(req.audio.max_correction_db, 4.0);
    Ok(())
}

#[test]
fn ming_request_round_trips() -> TestResult {
    let req: EngineRequest = round_trip(MING_REQUEST)?;
    check_request(&req, Engine::Ming);
    let EngineVoice::Reference { id, ref_key, transcript } = &req.voice else {
        return Err("expected a reference voice".into());
    };
    assert!(is_id(id));
    assert_eq!(ref_key, &format!("voices/{id}/ref.wav"));
    assert!(!transcript.is_empty());
    assert_eq!((req.audio.id, req.audio.bitrate_kbps, req.audio.sample_rate), (2, 56, 48_000));
    assert_eq!(req.audio.max_correction_db, 2.0);
    Ok(())
}

#[test]
fn engine_response_round_trips() -> TestResult {
    let res: EngineResponse = round_trip(RESPONSE)?;
    let req: EngineRequest = round_trip(KOKORO_REQUEST)?;
    assert_eq!((res.v, &res.job_id, res.engine), (CONTRACT_VERSION, &req.job_id, req.engine));
    let keys: Vec<&str> = res.items.iter().map(|i| i.key.as_str()).collect();
    let requested: Vec<&str> = req.items.iter().map(|i| i.key.as_str()).collect();
    assert_eq!(keys, requested);
    for (result, item) in res.items.iter().zip(&req.items) {
        assert_eq!(result.ok, result.error.is_none());
        if result.ok {
            assert_eq!(result.object_key, item.object_key);
            assert!(result.duration_ms > 0 && result.bytes > 0);
        }
        let text_chars = item.text.chars().count() as u32;
        for [start_ms, end_ms, char_start, char_end] in &result.words {
            assert!(start_ms <= end_ms && *end_ms <= result.duration_ms);
            assert!(char_start < char_end && *char_end <= text_chars);
        }
    }
    Ok(())
}

#[test]
fn prepare_reference_round_trips() -> TestResult {
    let req: PrepareReferenceRequest = round_trip(PREPARE_REQUEST)?;
    let res: PrepareReferenceResponse = round_trip(PREPARE_RESPONSE)?;
    assert_eq!((req.v, res.v), (CONTRACT_VERSION, CONTRACT_VERSION));
    assert_eq!(req.ref_key, res.ref_key);
    assert!(req.transcript.contains(&res.transcript), "kept span must come from the full transcript");
    assert!((10_000..=18_000).contains(&res.duration_ms));
    assert!(is_key(&res.sha256));
    Ok(())
}

#[test]
fn health_round_trips() -> TestResult {
    let health: EngineHealth = round_trip(HEALTH)?;
    assert_eq!(health.v, CONTRACT_VERSION);
    assert!(health.storage_ok);
    let engines: Vec<&str> = health.engines.keys().map(String::as_str).collect();
    assert_eq!(engines, ["kokoro", "ming"]);
    assert_eq!(health.model_revs.len(), 2);
    Ok(())
}

#[test]
fn round_trip_rejects_unknown_fields() -> TestResult {
    let with_extra = HEALTH.replacen("\"v\": 1,", "\"v\": 1, \"extra\": true,", 1);
    let typed: EngineHealth = serde_json::from_str(&with_extra)?;
    let again = serde_json::to_value(&typed)?;
    let original: Value = serde_json::from_str(&with_extra)?;
    assert_ne!(again, original, "the round-trip check would not notice a field the Rust type lacks");
    Ok(())
}

#[test]
fn every_fixture_is_checked() -> TestResult {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/fixtures");
    let present: BTreeSet<String> = std::fs::read_dir(&dir)?
        .filter_map(|entry| entry.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|name| name.ends_with(".json"))
        .collect();
    let checked: BTreeSet<String> = CHECKED.iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(present, checked);
    Ok(())
}
