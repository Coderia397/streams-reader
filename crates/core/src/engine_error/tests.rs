use super::*;

fn msg(message: &str) -> EngineError {
    EngineError::classify(None, message)
}

/// A trimmed copy of the Hugging Face proxy page served while a Space is asleep or restarting.
const HF_503_PAGE: &str = r#"
<!DOCTYPE html>
<html class="">
<head><meta charset="utf-8" /><title>503 Service Unavailable</title></head>
<body><h1>503</h1><p>This Space is currently unavailable.</p></body>
</html>"#;

#[test]
fn zerogpu_runs_limit() {
    assert_eq!(msg("You have exceeded your ZeroGPU runs limit."), EngineError::RunsLimit);
    assert_eq!(EngineError::classify(Some(429), "you have exceeded your zerogpu RUNS LIMIT"), EngineError::RunsLimit);
}

#[test]
fn zerogpu_quota_with_retry_time() {
    let err = msg("You have exceeded your free GPU quota (60s requested vs. 12s left). Try again in 1:02:03.");
    assert_eq!(err, EngineError::Quota { retry_after_s: Some(3723) });
    let err = msg("You have exceeded your GPU quota (60s requested vs. 12s left). Try again in 0:00:42");
    assert_eq!(err, EngineError::Quota { retry_after_s: Some(42) });
}

#[test]
fn zerogpu_quota_without_retry_time() {
    let err =
        msg("You have exceeded your GPU quota (150s requested vs. 149s left). Create a free account to get more.");
    assert_eq!(err, EngineError::Quota { retry_after_s: None });
}

#[test]
fn zerogpu_illegal_duration() {
    let err = msg("ZeroGPU illegal duration: The requested GPU duration (300s) is larger than the maximum allowed");
    assert_eq!(err, EngineError::IllegalDuration);
}

#[test]
fn budget_messages_win_over_http_status() {
    for status in [400, 401, 429, 500, 503] {
        assert_eq!(
            EngineError::classify(Some(status), "You have exceeded your ZeroGPU runs limit."),
            EngineError::RunsLimit,
            "status {status}"
        );
    }
}

#[test]
fn budget_messages_inside_gradio_json_and_sse_payloads() {
    let sse = r#"{"msg":"process_completed","success":false,"output":{"error":"You have exceeded your ZeroGPU runs limit."}}"#;
    assert_eq!(msg(sse), EngineError::RunsLimit);
    let quoted = "\"ZeroGPU illegal duration: The requested GPU duration (300s) is larger than the maximum allowed\"";
    assert_eq!(msg(quoted), EngineError::IllegalDuration);
}

#[test]
fn html_pages_mean_the_space_is_down() {
    assert_eq!(EngineError::classify(Some(503), HF_503_PAGE), EngineError::Sleeping);
    assert_eq!(EngineError::classify(Some(200), HF_503_PAGE), EngineError::Sleeping);
    assert_eq!(msg("<html><body>Building</body></html>"), EngineError::Sleeping);
}

#[test]
fn proxy_statuses_mean_the_space_is_down() {
    for status in [404, 502, 503, 504] {
        assert_eq!(EngineError::classify(Some(status), ""), EngineError::Sleeping, "status {status}");
        assert_eq!(EngineError::classify(Some(status), "{\"error\":\"x\"}"), EngineError::Sleeping);
    }
}

#[test]
fn space_state_messages_mean_the_space_is_down() {
    assert_eq!(msg("Space is sleeping"), EngineError::Sleeping);
    assert_eq!(msg("This Space has been paused by its owner."), EngineError::Sleeping);
    assert_eq!(msg("The space is restarting, try later"), EngineError::Sleeping);
}

#[test]
fn busy_gpu_and_full_queue() {
    assert_eq!(msg("No GPU is currently available for you after 60s"), EngineError::Busy);
    assert_eq!(msg("Queue is full! Please try again."), EngineError::Busy);
    assert_eq!(EngineError::classify(Some(429), "slow down"), EngineError::Busy);
}

#[test]
fn bad_request_keeps_the_detail() {
    assert_eq!(
        msg("bad request: items must be a list of 1 to 64 items"),
        EngineError::BadInput("items must be a list of 1 to 64 items".into())
    );
    assert_eq!(
        EngineError::classify(Some(500), "Error: Bad Request:   voice.ref_key must be under voices/ "),
        EngineError::BadInput("voice.ref_key must be under voices/".into())
    );
    // The detail keeps its original case and non-ASCII text.
    assert_eq!(msg("bad request: Ünïcode TEXT"), EngineError::BadInput("Ünïcode TEXT".into()));
}

#[test]
fn auth_failures() {
    assert_eq!(EngineError::classify(Some(401), "Unauthorized"), EngineError::Auth);
    assert_eq!(EngineError::classify(Some(403), HF_503_PAGE), EngineError::Auth);
}

#[test]
fn engine_errors_and_unknown_text_are_internal() {
    assert_eq!(msg("engine error: CUDA out of memory"), EngineError::Internal("CUDA out of memory".into()));
    assert_eq!(EngineError::classify(Some(500), "  boom  "), EngineError::Internal("boom".into()));
    assert_eq!(EngineError::classify(Some(500), ""), EngineError::Internal("HTTP 500".into()));
    assert_eq!(msg(""), EngineError::Internal("no error message".into()));
    // "starting" alone is not about the Space.
    assert_eq!(msg("starting tensor packing failed"), EngineError::Internal("starting tensor packing failed".into()));
}

#[test]
fn long_messages_are_clipped_on_a_char_boundary() {
    let long = "é".repeat(1000);
    let expected = format!("{}…", "é".repeat(MAX_MESSAGE_CHARS));
    assert_eq!(msg(&long), EngineError::Internal(expected));
}

#[test]
fn retry_after_parsing() {
    assert_eq!(retry_after_seconds("try again in 1:02:03."), Some(3723));
    assert_eq!(retry_after_seconds("try again in 0:00:42"), Some(42));
    assert_eq!(retry_after_seconds("try again in 12:34"), Some(754));
    assert_eq!(retry_after_seconds("try again in 90 seconds"), Some(90));
    assert_eq!(retry_after_seconds("try again in 23:59:59"), Some(86_399));
    assert_eq!(retry_after_seconds("try again in 1:2:3:4"), None);
    assert_eq!(retry_after_seconds("try again in 1::3"), None);
    assert_eq!(retry_after_seconds("try again in later"), None);
    assert_eq!(retry_after_seconds("try again in 99999999999999999999:00"), None);
    assert_eq!(retry_after_seconds("no hint"), None);
}

#[test]
fn transient_and_budget_policies() {
    let all = [
        EngineError::RunsLimit,
        EngineError::Quota { retry_after_s: None },
        EngineError::IllegalDuration,
        EngineError::Sleeping,
        EngineError::Busy,
        EngineError::BadInput(String::new()),
        EngineError::Auth,
        EngineError::Internal(String::new()),
    ];
    let transient: Vec<bool> = all.iter().map(EngineError::is_transient).collect();
    let budget: Vec<bool> = all.iter().map(EngineError::is_budget).collect();
    assert_eq!(transient, [false, false, false, true, true, false, false, true]);
    assert_eq!(budget, [true, true, true, false, false, false, false, false]);
}

#[test]
fn api_errors_carry_codes_and_retry_hints() {
    let quota = EngineError::Quota { retry_after_s: Some(60) }.to_api();
    assert_eq!((quota.code, quota.retry_after_s, quota.status()), (ErrorCode::GpuBudget, Some(60), 429));
    assert_eq!(EngineError::Quota { retry_after_s: None }.to_api().retry_after_s, None);
    assert_eq!(EngineError::Sleeping.to_api().code, ErrorCode::EngineUnavailable);
    assert_eq!(EngineError::Busy.to_api().status(), 503);
    let bad = EngineError::BadInput("text too long".into()).to_api();
    assert_eq!((bad.code, bad.message.as_str()), (ErrorCode::BadRequest, "bad request: text too long"));
    assert_eq!(EngineError::Auth.to_api().code, ErrorCode::EngineFailed);
    assert_eq!(EngineError::Internal("x".into()).to_api().message, "engine error: x");
}
