use super::*;

const NOW: i64 = 1_800_000_000_000;
const HOUR_MS: i64 = 60 * 60 * 1000;

fn used(at_ms: i64, requested_s: u32, used_s: u32) -> GpuUse {
    GpuUse { at_ms, requested_s, used_s }
}

fn policy() -> GpuPolicy {
    GpuPolicy::default()
}

#[test]
fn default_policy_matches_notes_08() {
    let p = policy();
    assert_eq!((p.daily_cap_s, p.runs_cap, p.base_s, p.max_call_s), (280, 3, 8, 180));
    assert_eq!((p.duration_factor, p.s_per_char), (1.5, 0.06));
    // The largest call still passes admission on a fresh window: 180 * 1.5 = 270 <= 280.
    assert!(charged(&p, p.max_call_s) <= p.daily_cap_s);
}

#[test]
fn estimate_is_base_plus_per_char_rounded_up() {
    let p = policy();
    assert_eq!(estimate_seconds(&p, 0), 8);
    assert_eq!(estimate_seconds(&p, 1), 9);
    assert_eq!(estimate_seconds(&p, 100), 14);
    // The measured Ming call: 499 chars took 25.8 GPU-s; the estimate leaves margin.
    assert_eq!(estimate_seconds(&p, 499), 38);
}

#[test]
fn estimate_is_capped_at_max_call() {
    let p = policy();
    assert_eq!(estimate_seconds(&p, 2_866), 180);
    assert_eq!(estimate_seconds(&p, 2_867), 180);
    assert_eq!(estimate_seconds(&p, usize::MAX), 180);
}

#[test]
fn estimate_does_not_round_float_noise_up() {
    let p = GpuPolicy { base_s: 0, s_per_char: 0.07, ..policy() };
    assert_eq!(estimate_seconds(&p, 100), 7);
    let p = GpuPolicy { base_s: 0, s_per_char: 0.1, ..policy() };
    assert_eq!(estimate_seconds(&p, 30), 3);
    assert_eq!(estimate_seconds(&p, 31), 4);
}

#[test]
fn charge_applies_the_duration_factor_rounded_up() {
    let p = policy();
    assert_eq!(charged(&p, 20), 30);
    assert_eq!(charged(&p, 21), 32);
    assert_eq!(charged(&p, 0), 0);
    assert_eq!(charged(&p, u32::MAX), u32::MAX);
}

#[test]
fn empty_history_is_a_fresh_window() {
    let v = view(&policy(), &[], NOW);
    assert_eq!(v, BudgetView { used_s: 0, runs_used: 0, remaining_s: 280, runs_left: 3, resets_at_ms: None });
}

#[test]
fn view_charges_the_larger_of_used_and_scaled_request() {
    let history = [used(NOW - HOUR_MS, 20, 25), used(NOW - 2 * HOUR_MS, 20, 40)];
    let v = view(&policy(), &history, NOW);
    assert_eq!((v.used_s, v.runs_used, v.remaining_s, v.runs_left), (70, 2, 210, 1));
    assert_eq!(v.resets_at_ms, Some(NOW - 2 * HOUR_MS + DAY_MS));
}

#[test]
fn failed_calls_still_count_their_request() {
    let v = view(&policy(), &[used(NOW - 1, 60, 0)], NOW);
    assert_eq!((v.used_s, v.runs_used), (90, 1));
}

#[test]
fn window_ignores_uses_older_than_a_day() {
    let history = [used(NOW - DAY_MS, 100, 100), used(NOW - DAY_MS + 1, 10, 10), used(NOW - 3 * DAY_MS, 100, 100)];
    let v = view(&policy(), &history, NOW);
    assert_eq!((v.used_s, v.runs_used), (15, 1));
    assert_eq!(v.resets_at_ms, Some(NOW + 1));
}

#[test]
fn window_starts_at_the_earliest_use_regardless_of_order() {
    let history = [used(NOW - HOUR_MS, 1, 1), used(NOW - 5 * HOUR_MS, 1, 1), used(NOW - 3 * HOUR_MS, 1, 1)];
    assert_eq!(view(&policy(), &history, NOW).resets_at_ms, Some(NOW - 5 * HOUR_MS + DAY_MS));
}

#[test]
fn future_uses_from_clock_skew_count() {
    let v = view(&policy(), &[used(NOW + 5_000, 10, 10)], NOW);
    assert_eq!((v.used_s, v.runs_used, v.resets_at_ms), (15, 1, Some(NOW + 5_000 + DAY_MS)));
}

#[test]
fn overspending_saturates_at_zero() {
    let history = [used(NOW - 1, 200, 200), used(NOW - 2, 200, 200), used(NOW - 3, 1, 1), used(NOW - 4, 1, 1)];
    let v = view(&policy(), &history, NOW);
    assert_eq!((v.remaining_s, v.runs_left, v.runs_used), (0, 0, 4));
}

#[test]
fn admits_a_batch_that_fits() {
    // 200 chars -> 20 s requested -> 30 s charged.
    assert_eq!(admit(&policy(), &[], NOW, &[100, 100]), Admission::Admit { request_s: 20 });
}

#[test]
fn admits_an_empty_batch_with_no_request() {
    assert_eq!(admit(&policy(), &[], NOW, &[]), Admission::Admit { request_s: 0 });
}

#[test]
fn shrinks_to_the_longest_prefix_within_remaining_seconds() {
    // 250 s used leaves 30 s: 200 chars -> 20 s -> 30 s fits exactly; 300 chars -> 26 s -> 39 s does not.
    let history = [used(NOW - HOUR_MS, 0, 250)];
    assert_eq!(admit(&policy(), &history, NOW, &[100, 100, 100]), Admission::Shrink { items: 2, request_s: 20 });
}

#[test]
fn shrinks_a_batch_longer_than_one_call() {
    // 20 items of 500 chars: k items need 8 + 30k s, so 5 items (158 s, charged 237 s) is the most one call can take.
    let items = [500; 20];
    assert_eq!(admit(&policy(), &[], NOW, &items), Admission::Shrink { items: 5, request_s: 158 });
}

#[test]
fn a_single_oversized_item_is_admitted_at_the_cap() {
    assert_eq!(admit(&policy(), &[], NOW, &[5_000]), Admission::Admit { request_s: 180 });
    assert_eq!(admit(&policy(), &[], NOW, &[5_000, 10]), Admission::Shrink { items: 1, request_s: 180 });
}

#[test]
fn defers_to_the_window_reset_when_nothing_fits() {
    let first = NOW - 2 * HOUR_MS;
    let history = [used(first, 0, 270)];
    assert_eq!(admit(&policy(), &history, NOW, &[100]), Admission::Defer { retry_at_ms: first + DAY_MS });
}

#[test]
fn defers_when_no_runs_are_left() {
    let first = NOW - 3 * HOUR_MS;
    let history = [used(first, 1, 1), used(NOW - 2 * HOUR_MS, 1, 1), used(NOW - HOUR_MS, 1, 1)];
    assert_eq!(admit(&policy(), &history, NOW, &[1]), Admission::Defer { retry_at_ms: first + DAY_MS });
}

#[test]
fn defers_a_full_day_when_the_reset_is_unknown() {
    let p = GpuPolicy { runs_cap: 0, ..policy() };
    assert_eq!(admit(&p, &[], NOW, &[1]), Admission::Defer { retry_at_ms: NOW + DAY_MS });
    let p = GpuPolicy { daily_cap_s: 0, ..policy() };
    assert_eq!(admit(&p, &[], NOW, &[1]), Admission::Defer { retry_at_ms: NOW + DAY_MS });
}

#[test]
fn a_window_that_expired_frees_the_budget() {
    let history = [used(NOW - DAY_MS - 1, 180, 180), used(NOW - DAY_MS - 2, 1, 1), used(NOW - DAY_MS - 3, 1, 1)];
    assert_eq!(admit(&policy(), &history, NOW, &[100]), Admission::Admit { request_s: 14 });
}

#[test]
fn a_whole_day_of_calls_stays_under_the_cap() {
    // Simulate a day of greedy admission against the ledger: the charged total never exceeds the cap.
    let p = policy();
    let mut history = Vec::new();
    let mut now = NOW;
    let mut queue: Vec<usize> = vec![450; 40];
    while !queue.is_empty() {
        match admit(&p, &history, now, &queue) {
            Admission::Admit { request_s } => {
                history.push(used(now, request_s, request_s));
                queue.clear();
            }
            Admission::Shrink { items, request_s } => {
                history.push(used(now, request_s, request_s));
                queue.drain(..items);
            }
            Admission::Defer { retry_at_ms } => {
                assert!(retry_at_ms > now);
                let v = view(&p, &history, now);
                assert!(v.used_s <= p.daily_cap_s, "{v:?}");
                now = retry_at_ms;
            }
        }
        now += 60_000;
    }
}

// ------------------------------------------------------------------------------------------------- per-user shares

#[test]
fn a_user_share_caps_their_calls_below_the_global_budget() {
    let p = policy();
    // 60 s share: one 499-char item is 38 s requested, 57 s charged; a second does not fit.
    let share = Share { daily_s: 60, history: &[] };
    assert_eq!(admit_for_user(&p, &[], share, NOW, &[499]), Admission::Admit { request_s: 38 });
    assert_eq!(admit_for_user(&p, &[], share, NOW, &[499, 499]), Admission::Shrink { items: 1, request_s: 38 });
}

#[test]
fn a_spent_share_defers_to_the_users_own_reset() {
    let p = policy();
    let mine = [used(NOW - HOUR_MS, 38, 40)];
    // Globally there is room (someone else's use is older), but this user's 60 s are spent.
    let all = [used(NOW - 5 * HOUR_MS, 10, 10), mine[0]];
    let got = admit_for_user(&p, &all, Share { daily_s: 60, history: &mine }, NOW, &[100]);
    assert_eq!(got, Admission::Defer { retry_at_ms: NOW - HOUR_MS + DAY_MS });
}

#[test]
fn the_global_budget_still_binds_a_large_share() {
    let p = policy();
    // 270 s charged leaves 10 s; 100 chars request 14 s, charged 21 s.
    let all = [used(NOW - 3 * HOUR_MS, 170, 270)];
    let got = admit_for_user(&p, &all, Share { daily_s: 280, history: &[] }, NOW, &[100]);
    assert_eq!(got, Admission::Defer { retry_at_ms: NOW - 3 * HOUR_MS + DAY_MS });
}

#[test]
fn runs_are_shared_by_everyone() {
    let p = policy();
    let all = [used(NOW - 3, 9, 9), used(NOW - 2, 9, 9), used(NOW - 1, 9, 9)];
    let got = admit_for_user(&p, &all, Share { daily_s: 280, history: &[] }, NOW, &[10]);
    assert!(matches!(got, Admission::Defer { .. }));
}

#[test]
fn a_zero_share_never_admits_and_waits_a_day() {
    let got = admit_for_user(&policy(), &[], Share { daily_s: 0, history: &[] }, NOW, &[10]);
    assert_eq!(got, Admission::Defer { retry_at_ms: NOW + DAY_MS });
}

#[test]
fn a_full_share_matches_plain_admit() {
    let p = policy();
    let history = [used(NOW - HOUR_MS, 20, 25)];
    for chars in [vec![100], vec![2000, 2000], vec![]] {
        assert_eq!(
            admit_for_user(&p, &history, Share { daily_s: p.daily_cap_s, history: &history }, NOW, &chars),
            admit(&p, &history, NOW, &chars)
        );
    }
}
