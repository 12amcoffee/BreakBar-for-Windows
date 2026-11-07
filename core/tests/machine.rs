//! Port of the Mac app's TimerMachineTests (same scenarios, same numbers).
use breakbar_core::*;
use TimerEffect::*;
use TimerEvent::*;

const T0: Time = 1_000_000.0;
const MIN: f64 = 60.0;

fn mk(tweak: impl FnOnce(&mut TimerSettings)) -> TimerMachine {
    let mut s = TimerSettings::default();
    tweak(&mut s);
    TimerMachine::new(s)
}

/// Runs one full work session and returns the time work ended.
fn finish_work(m: &mut TimerMachine, from: Time) -> Time {
    m.handle(Start, from);
    let end = from + f64::from(m.settings.work_minutes) * MIN;
    m.handle(Tick, end);
    end
}

fn is_working(m: &TimerMachine) -> bool {
    matches!(m.state(), TimerState::Working { .. })
}
fn is_resting(m: &TimerMachine) -> bool {
    matches!(m.state(), TimerState::Resting { .. })
}
fn auto_rest(s: &mut TimerSettings) {
    s.when_work_ends = WhenWorkEnds::StartRestAutomatically;
}

// Basics

#[test]
fn start_begins_work_with_sounds() {
    let mut m = mk(|_| {});
    let fx = m.handle(Start, T0);
    assert_eq!(m.state(), TimerState::Working { ends_at: T0 + 45.0 * MIN });
    assert_eq!(fx, vec![PlayWindup, StartTicking]);
}

#[test]
fn tick_before_end_does_nothing() {
    let mut m = mk(|_| {});
    m.handle(Start, T0);
    assert_eq!(m.handle(Tick, T0 + 60.0), vec![]);
}

#[test]
fn work_end_shows_rest_window() {
    let mut m = mk(|_| {});
    let end = finish_work(&mut m, T0);
    assert_eq!(m.state(), TimerState::AwaitingRest { kind: RestKind::Short, since: end, snoozes: 0 });
    assert_eq!(m.completed_sessions(), 1);
}

#[test]
fn work_end_emits_ding_and_notification() {
    let mut m = mk(|_| {});
    m.handle(Start, T0);
    let fx = m.handle(Tick, T0 + 45.0 * MIN);
    assert_eq!(fx, vec![StopTicking, PlayDing, NotifyRestStarted(RestKind::Short)]);
}

#[test]
fn auto_rest_starts_immediately() {
    let mut m = mk(auto_rest);
    let end = finish_work(&mut m, T0);
    assert_eq!(m.state(), TimerState::Resting { kind: RestKind::Short, ends_at: end + 10.0 * MIN });
}

#[test]
fn start_rest_button_starts_rest_timer_from_click() {
    let mut m = mk(|_| {});
    let end = finish_work(&mut m, T0);
    let click = end + 90.0;
    m.handle(StartRest, click);
    assert_eq!(m.state(), TimerState::Resting { kind: RestKind::Short, ends_at: click + 10.0 * MIN });
}

#[test]
fn stop_goes_idle_and_keeps_counter() {
    let mut m = mk(|_| {});
    finish_work(&mut m, T0);
    m.handle(Stop, T0 + 3000.0);
    assert_eq!(m.state(), TimerState::Idle);
    assert_eq!(m.completed_sessions(), 1);
}

#[test]
fn stop_during_work_stops_ticking() {
    let mut m = mk(|_| {});
    m.handle(Start, T0);
    assert_eq!(m.handle(Stop, T0 + 10.0), vec![StopTicking]);
}

// Rest end

#[test]
fn rest_end_shows_work_window_by_default() {
    let mut m = mk(auto_rest);
    let end = finish_work(&mut m, T0);
    let rest_end = end + 600.0;
    let fx = m.handle(Tick, rest_end);
    assert_eq!(m.state(), TimerState::AwaitingWork { kind: RestKind::Short, since: rest_end });
    assert_eq!(fx, vec![NotifyRestEnded]);
}

#[test]
fn rest_end_auto_starts_work() {
    let mut m = mk(|s| {
        auto_rest(s);
        s.when_rest_ends = WhenRestEnds::StartWorkAutomatically;
    });
    let end = finish_work(&mut m, T0);
    let fx = m.handle(Tick, end + 600.0);
    assert_eq!(fx, vec![NotifyRestEnded, PlayWindup, StartTicking]);
    assert!(is_working(&m));
}

#[test]
fn rest_end_stop_timer() {
    let mut m = mk(|s| {
        auto_rest(s);
        s.when_rest_ends = WhenRestEnds::StopTimer;
    });
    let end = finish_work(&mut m, T0);
    m.handle(Tick, end + 600.0);
    assert_eq!(m.state(), TimerState::Idle);
}

#[test]
fn continuous_cycles_overrides_both_windows() {
    let mut m = mk(|s| s.continuous_cycles = true);
    let end = finish_work(&mut m, T0);
    assert!(is_resting(&m));
    m.handle(Tick, end + 600.0);
    assert!(is_working(&m));
}

#[test]
fn start_work_button() {
    let mut m = mk(auto_rest);
    let end = finish_work(&mut m, T0);
    m.handle(Tick, end + 600.0);
    let click = end + 700.0;
    let fx = m.handle(StartWork, click);
    assert_eq!(m.state(), TimerState::Working { ends_at: click + 45.0 * MIN });
    assert_eq!(fx, vec![PlayWindup, StartTicking]);
}

// Long break counter

#[test]
fn long_break_after_n_sessions() {
    let mut m = mk(|s| {
        s.sessions_per_set = 2;
        auto_rest(s);
        s.when_rest_ends = WhenRestEnds::StartWorkAutomatically;
    });
    let mut now = T0;
    m.handle(Start, now);
    now += 45.0 * MIN;
    m.handle(Tick, now); // session 1 -> short
    assert_eq!(m.state(), TimerState::Resting { kind: RestKind::Short, ends_at: now + 600.0 });
    now += 600.0;
    m.handle(Tick, now); // -> working
    now += 45.0 * MIN;
    m.handle(Tick, now); // session 2 -> long
    assert_eq!(m.state(), TimerState::Resting { kind: RestKind::Long, ends_at: now + 20.0 * MIN });
    assert_eq!(m.completed_sessions(), 0); // set reset
}

#[test]
fn skipping_long_break_still_resets_set() {
    let mut m = mk(|s| s.sessions_per_set = 1);
    let end = finish_work(&mut m, T0);
    assert_eq!(m.state(), TimerState::AwaitingRest { kind: RestKind::Long, since: end, snoozes: 0 });
    m.handle(Skip, end + 5.0);
    assert_eq!(m.completed_sessions(), 0);
    assert!(is_working(&m));
}

#[test]
fn skip_short_break_keeps_counter() {
    let mut m = mk(|_| {});
    let end = finish_work(&mut m, T0);
    m.handle(Skip, end + 5.0);
    assert_eq!(m.completed_sessions(), 1);
}

#[test]
fn delay_long_break_postpones_once() {
    let mut m = mk(|s| {
        s.sessions_per_set = 1;
        s.delay_long_break = true;
    });
    let mut end = finish_work(&mut m, T0);
    assert_eq!(m.state(), TimerState::AwaitingRest { kind: RestKind::Short, since: end, snoozes: 0 }); // postponed
    assert!(m.long_break_postponed());
    m.handle(Skip, end); // back to work
    let work2 = end;
    end = work2 + 45.0 * MIN;
    m.handle(Tick, end);
    assert_eq!(m.state(), TimerState::AwaitingRest { kind: RestKind::Long, since: end, snoozes: 0 }); // now long
    assert!(!m.long_break_postponed());
    assert_eq!(m.completed_sessions(), 0);
}

#[test]
fn reset_set_manual() {
    let mut m = mk(|_| {});
    finish_work(&mut m, T0);
    m.handle(ResetSet, T0);
    assert_eq!(m.completed_sessions(), 0);
}

#[test]
fn inactivity_resets_set_on_next_start() {
    let mut m = mk(|_| {});
    let end = finish_work(&mut m, T0);
    m.handle(Stop, end);
    m.handle(Start, end + 61.0 * MIN); // > 60 min later
    assert_eq!(m.completed_sessions(), 0);
}

#[test]
fn short_gap_keeps_set() {
    let mut m = mk(|_| {});
    let end = finish_work(&mut m, T0);
    m.handle(Stop, end);
    m.handle(Start, end + 30.0 * MIN);
    assert_eq!(m.completed_sessions(), 1);
}

#[test]
fn unanswered_rest_window_goes_idle_and_resets_set() {
    let mut m = mk(|_| {});
    let end = finish_work(&mut m, T0);
    m.handle(Tick, end + 61.0 * MIN);
    assert_eq!(m.state(), TimerState::Idle);
    assert_eq!(m.completed_sessions(), 0);
}

// Snooze

#[test]
fn snooze_then_window_returns() {
    let mut m = mk(|_| {});
    let end = finish_work(&mut m, T0);
    m.handle(Snooze, end);
    assert_eq!(m.state(), TimerState::Snoozed { kind: RestKind::Short, until: end + 120.0, snoozes: 1 });
    m.handle(Tick, end + 119.0);
    assert!(matches!(m.state(), TimerState::Snoozed { .. }));
    let back = end + 121.0;
    m.handle(Tick, back);
    assert_eq!(m.state(), TimerState::AwaitingRest { kind: RestKind::Short, since: back, snoozes: 1 });
}

#[test]
fn snooze_limit() {
    let mut m = mk(|s| s.max_snoozes_per_break = 2);
    let mut now = finish_work(&mut m, T0);
    for _ in 0..2 {
        m.handle(Snooze, now);
        now += 121.0;
        m.handle(Tick, now);
    }
    m.handle(Snooze, now); // third: refused
    assert_eq!(m.state(), TimerState::AwaitingRest { kind: RestKind::Short, since: now, snoozes: 2 });
}

#[test]
fn snooze_disabled() {
    let mut m = mk(|s| s.snooze_enabled = false);
    let end = finish_work(&mut m, T0);
    m.handle(Snooze, end);
    assert_eq!(m.state(), TimerState::AwaitingRest { kind: RestKind::Short, since: end, snoozes: 0 });
}

#[test]
fn snooze_does_not_change_counter() {
    let mut m = mk(|_| {});
    let end = finish_work(&mut m, T0);
    m.handle(Snooze, end);
    assert_eq!(m.completed_sessions(), 1);
}

#[test]
fn start_rest_from_snoozed_works() {
    let mut m = mk(|_| {});
    let end = finish_work(&mut m, T0);
    m.handle(Snooze, end);
    m.handle(StartRest, end + 30.0);
    assert!(is_resting(&m), "got {:?}", m.state());
}

// Extend & skip

#[test]
fn extend_rest() {
    let mut m = mk(auto_rest);
    let end = finish_work(&mut m, T0);
    m.handle(Extend, end + 100.0);
    assert_eq!(m.state(), TimerState::Resting { kind: RestKind::Short, ends_at: end + 600.0 + 300.0 });
    m.handle(Extend, end + 200.0);
    assert_eq!(m.state(), TimerState::Resting { kind: RestKind::Short, ends_at: end + 600.0 + 600.0 });
}

#[test]
fn extend_from_work_window_reenters_rest() {
    let mut m = mk(auto_rest);
    let end = finish_work(&mut m, T0);
    let rest_end = end + 600.0;
    m.handle(Tick, rest_end);
    let click = rest_end + 20.0;
    m.handle(Extend, click);
    assert_eq!(m.state(), TimerState::Resting { kind: RestKind::Short, ends_at: click + 300.0 });
}

#[test]
fn extend_disabled() {
    let mut m = mk(|s| {
        auto_rest(s);
        s.extend_enabled = false;
    });
    let end = finish_work(&mut m, T0);
    let before = m.state();
    m.handle(Extend, end + 10.0);
    assert_eq!(m.state(), before);
}

#[test]
fn skip_resting_goes_to_work() {
    let mut m = mk(auto_rest);
    let end = finish_work(&mut m, T0);
    let fx = m.handle(Skip, end + 10.0);
    assert_eq!(fx, vec![PlayWindup, StartTicking]);
    assert_eq!(m.completed_sessions(), 1);
}

// Pause

#[test]
fn pause_and_resume_work() {
    let mut m = mk(|_| {});
    m.handle(Start, T0);
    let p = T0 + 600.0;
    assert_eq!(m.handle(Pause, p), vec![StopTicking]);
    assert_eq!(m.state(), TimerState::Paused { phase: TimedPhase::Work, remaining: 35.0 * MIN, since: p });
    let r = p + 5000.0;
    m.handle(Resume, r);
    assert_eq!(m.state(), TimerState::Working { ends_at: r + 35.0 * MIN });
}

#[test]
fn pause_does_not_tick() {
    let mut m = mk(|_| {});
    m.handle(Start, T0);
    m.handle(Pause, T0 + 10.0);
    m.handle(Tick, T0 + 45.0 * MIN + 5.0);
    assert!(matches!(m.state(), TimerState::Paused { .. }));
}

#[test]
fn pause_rest_and_resume() {
    let mut m = mk(auto_rest);
    let end = finish_work(&mut m, T0);
    let p = end + 100.0;
    m.handle(Pause, p);
    assert_eq!(
        m.state(),
        TimerState::Paused { phase: TimedPhase::Rest(RestKind::Short), remaining: 500.0, since: p }
    );
    let r = p + 60.0;
    assert_eq!(m.handle(Resume, r), vec![]);
    assert_eq!(m.state(), TimerState::Resting { kind: RestKind::Short, ends_at: r + 500.0 });
}

#[test]
fn pause_disabled() {
    let mut m = mk(|s| s.pause_enabled = false);
    m.handle(Start, T0);
    m.handle(Pause, T0 + 5.0);
    assert!(is_working(&m));
}

#[test]
fn paused_too_long_goes_idle() {
    let mut m = mk(|_| {});
    m.handle(Start, T0);
    m.handle(Pause, T0 + 5.0);
    m.handle(Tick, T0 + 5.0 + 61.0 * MIN);
    assert_eq!(m.state(), TimerState::Idle);
}

// Sleep protection

#[test]
fn late_work_tick_after_sleep_goes_idle_without_counting() {
    let mut m = mk(|_| {});
    m.handle(Start, T0);
    m.handle(Tick, T0 + 45.0 * MIN + 61.0);
    assert_eq!(m.state(), TimerState::Idle);
    assert_eq!(m.completed_sessions(), 0);
}

#[test]
fn slightly_late_tick_still_counts() {
    let mut m = mk(|_| {});
    m.handle(Start, T0);
    m.handle(Tick, T0 + 45.0 * MIN + 30.0);
    assert_eq!(m.completed_sessions(), 1);
}

#[test]
fn late_rest_tick_after_sleep_goes_idle() {
    let mut m = mk(auto_rest);
    let end = finish_work(&mut m, T0);
    m.handle(Tick, end + 600.0 + 120.0);
    assert_eq!(m.state(), TimerState::Idle);
}

// Settings

#[test]
fn settings_are_clamped() {
    let s = TimerSettings { work_minutes: 0, sessions_per_set: 99, long_rest_minutes: 999, ..Default::default() };
    let c = s.clamped();
    assert_eq!(c.work_minutes, 1);
    assert_eq!(c.sessions_per_set, 12);
    assert_eq!(c.long_rest_minutes, 180);
}

#[test]
fn settings_json_round_trip() {
    let s = TimerSettings { work_minutes: 50, delay_long_break: true, ..Default::default() };
    let back: TimerSettings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(s, back);
}

#[test]
fn old_or_partial_settings_still_decode() {
    let s: TimerSettings = serde_json::from_str(r#"{"workMinutes": 50, "snoozeEnabled": false}"#).unwrap();
    assert_eq!(s.work_minutes, 50);
    assert!(!s.snooze_enabled);
    assert_eq!(s.short_rest_minutes, 10); // default filled in
    assert_eq!(serde_json::from_str::<TimerSettings>("{}").unwrap(), TimerSettings::default());
}

// Extra: the JSON must stay readable by (and identical in shape to) the Mac app's.

#[test]
fn json_uses_the_mac_apps_key_names() {
    let v = serde_json::to_value(TimerSettings::default()).unwrap();
    assert_eq!(v["workMinutes"], 45);
    assert_eq!(v["whenWorkEnds"], "showRestWindow");
    assert_eq!(v["windowStyle"], "small");
    assert!(v.get("work_minutes").is_none());
}

#[test]
fn window_style_full_screen_value() {
    let s: TimerSettings = serde_json::from_str(r#"{"windowStyle":"fullScreen"}"#).unwrap();
    assert_eq!(s.window_style, PromptWindowStyle::FullScreen);
}
