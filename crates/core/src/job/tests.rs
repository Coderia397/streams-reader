use super::*;

const NOW: i64 = 1_800_000_000_000;

fn row(state: RenderState, engine: Engine, attempts: u32) -> Row {
    Row { state, engine, attempts, lease_until_ms: None, retry_at_ms: None }
}

fn rendering(engine: Engine, attempts: u32) -> Row {
    Row { lease_until_ms: Some(NOW - 1), ..row(RenderState::Rendering, engine, attempts) }
}

fn fail(error: EngineError) -> Event {
    Event::Fail { error, now_ms: NOW }
}

#[test]
fn start_claims_a_queued_row_with_a_lease() {
    let queued = Row { retry_at_ms: Some(NOW - 5), ..row(RenderState::Queued, Engine::Kokoro, 1) };
    let got = apply(&queued, &Event::Start { now_ms: NOW });
    assert_eq!(
        got,
        Ok(Row {
            state: RenderState::Rendering,
            engine: Engine::Kokoro,
            attempts: 2,
            lease_until_ms: Some(NOW + LEASE_MS),
            retry_at_ms: None,
        })
    );
}

#[test]
fn done_makes_the_row_ready() {
    for engine in [Engine::Kokoro, Engine::Ming] {
        let got = apply(&rendering(engine, 1), &Event::Done);
        assert_eq!(got, Ok(row(RenderState::Ready, engine, 1)));
    }
}

#[test]
fn budget_failures_defer_until_the_quota_frees_up() {
    for engine in [Engine::Kokoro, Engine::Ming] {
        let got = apply(&rendering(engine, 1), &fail(EngineError::Quota { retry_after_s: Some(3723) }));
        assert_eq!(got, Ok(Row { retry_at_ms: Some(NOW + 3_723_000), ..row(RenderState::Deferred, engine, 1) }));
    }
}

#[test]
fn budget_failures_without_a_time_defer_one_hour() {
    for error in [EngineError::RunsLimit, EngineError::IllegalDuration, EngineError::Quota { retry_after_s: None }] {
        let got = apply(&rendering(Engine::Ming, 1), &fail(error));
        assert_eq!(got, Ok(Row { retry_at_ms: Some(NOW + 3_600_000), ..row(RenderState::Deferred, Engine::Ming, 1) }));
    }
}

#[test]
fn huge_retry_after_saturates() {
    let got = apply(&rendering(Engine::Ming, 1), &fail(EngineError::Quota { retry_after_s: Some(u64::MAX) }));
    assert_eq!(got.map(|r| r.retry_at_ms), Ok(Some(i64::MAX)));
}

#[test]
fn transient_kokoro_failures_requeue_with_exponential_backoff() {
    let transient = [EngineError::Sleeping, EngineError::Busy, EngineError::Internal("x".into())];
    for (attempts, wait_ms) in [(1, 30_000), (2, 60_000), (3, 120_000)] {
        for error in transient.clone() {
            let got = apply(&rendering(Engine::Kokoro, attempts), &fail(error));
            let expected =
                Row { retry_at_ms: Some(NOW + wait_ms), ..row(RenderState::Queued, Engine::Kokoro, attempts) };
            assert_eq!(got, Ok(expected), "attempt {attempts}");
        }
    }
}

#[test]
fn transient_kokoro_failures_fail_after_the_last_attempt() {
    let got = apply(&rendering(Engine::Kokoro, MAX_CPU_ATTEMPTS), &fail(EngineError::Busy));
    assert_eq!(got, Ok(row(RenderState::Failed, Engine::Kokoro, MAX_CPU_ATTEMPTS)));
}

#[test]
fn permanent_failures_fail() {
    for error in [EngineError::BadInput("x".into()), EngineError::Auth] {
        let got = apply(&rendering(Engine::Kokoro, 1), &fail(error));
        assert_eq!(got, Ok(row(RenderState::Failed, Engine::Kokoro, 1)));
    }
}

#[test]
fn ming_never_retries_automatically() {
    for error in [EngineError::Sleeping, EngineError::Busy, EngineError::Internal("x".into()), EngineError::Auth] {
        let got = apply(&rendering(Engine::Ming, 1), &fail(error));
        assert_eq!(got, Ok(row(RenderState::Failed, Engine::Ming, 1)));
    }
}

#[test]
fn expired_leases_requeue_cpu_jobs_and_fail_gpu_jobs() {
    let got = apply(&rendering(Engine::Kokoro, 2), &Event::LeaseExpired);
    assert_eq!(got, Ok(row(RenderState::Queued, Engine::Kokoro, 2)));
    let got = apply(&rendering(Engine::Ming, 1), &Event::LeaseExpired);
    assert_eq!(got, Ok(row(RenderState::Failed, Engine::Ming, 1)));
}

#[test]
fn expired_leases_stop_requeueing_after_the_last_attempt() {
    let got = apply(&rendering(Engine::Kokoro, MAX_CPU_ATTEMPTS), &Event::LeaseExpired);
    assert_eq!(got, Ok(row(RenderState::Failed, Engine::Kokoro, MAX_CPU_ATTEMPTS)));
}

#[test]
fn due_requeues_a_deferred_row() {
    let deferred = Row { retry_at_ms: Some(NOW), ..row(RenderState::Deferred, Engine::Ming, 1) };
    assert_eq!(apply(&deferred, &Event::Due), Ok(row(RenderState::Queued, Engine::Ming, 1)));
}

#[test]
fn a_kokoro_job_cannot_loop_forever() -> Result<(), String> {
    let mut r = row(RenderState::Queued, Engine::Kokoro, 0);
    let mut steps = 0;
    while r.state != RenderState::Failed {
        let event = if r.state == RenderState::Queued { Event::Start { now_ms: NOW } } else { Event::LeaseExpired };
        r = apply(&r, &event)?;
        steps += 1;
        assert!(steps < 20, "no terminal state after {steps} steps");
    }
    assert_eq!(r.attempts, MAX_CPU_ATTEMPTS);
    Ok(())
}

#[test]
fn every_other_transition_is_rejected() {
    use RenderState::*;
    let events = [
        Event::Start { now_ms: NOW },
        Event::Done,
        Event::Fail { error: EngineError::Busy, now_ms: NOW },
        Event::LeaseExpired,
        Event::Due,
    ];
    let allowed = |state: RenderState, event: &Event| {
        matches!(
            (state, event),
            (Queued, Event::Start { .. })
                | (Rendering, Event::Done | Event::Fail { .. } | Event::LeaseExpired)
                | (Deferred, Event::Due)
        )
    };
    let mut rejected = 0;
    for state in [Ready, Queued, Rendering, Deferred, Failed] {
        for engine in [Engine::Kokoro, Engine::Ming] {
            for event in &events {
                let r = Row { lease_until_ms: Some(7), retry_at_ms: Some(9), ..row(state, engine, 1) };
                match apply(&r, event) {
                    Ok(_) => assert!(allowed(state, event), "{state:?} + {event:?} should be rejected"),
                    Err(err) => {
                        assert!(!allowed(state, event), "{state:?} + {event:?} should be allowed: {err}");
                        assert!(err.contains(&format!("{state:?}")), "{err}");
                        rejected += 1;
                    }
                }
            }
        }
    }
    assert_eq!(rejected, 2 * (25 - 5));
}

#[test]
fn rejection_messages_name_the_event_and_state() {
    let err = apply(&row(RenderState::Ready, Engine::Kokoro, 1), &Event::Done);
    assert_eq!(err, Err("cannot apply Done to a Ready row".to_owned()));
}
