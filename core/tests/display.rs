//! Port of the Mac app's DisplayTests, CommandAndNameTests and DotsTests.
use breakbar_core::*;
use TimerEvent::*;

const T0: Time = 1_000_000.0;
const MIN: f64 = 60.0;

fn started() -> TimerMachine {
    let mut m = TimerMachine::default();
    m.handle(Start, T0);
    m
}

#[test]
fn format_clock_cases() {
    assert_eq!(format_clock(0), "00:00");
    assert_eq!(format_clock(65), "01:05");
    assert_eq!(format_clock(45 * 60), "45:00");
    assert_eq!(format_clock(-3), "00:00");
}

#[test]
fn idle_has_no_title() {
    assert_eq!(TimerMachine::default().pill_text(T0), None);
    assert_eq!(TimerMachine::default().icon(), PillIcon::Idle);
}

#[test]
fn work_countdown_starts_at_full_length() {
    let m = started();
    assert_eq!(m.pill_text(T0).as_deref(), Some("45:00"));
    assert_eq!(m.pill_text(T0 + 0.4).as_deref(), Some("45:00"));
    assert_eq!(m.pill_text(T0 + 61.0).as_deref(), Some("43:59"));
    assert_eq!(m.icon(), PillIcon::Working);
}

#[test]
fn waiting_to_rest_counts_up_with_plus() {
    let mut m = started();
    let end = T0 + 45.0 * MIN;
    m.handle(Tick, end);
    assert_eq!(m.icon(), PillIcon::WaitingToRest);
    assert_eq!(m.pill_text(end + 83.0).as_deref(), Some("+01:23"));
}

#[test]
fn snoozed_title() {
    let mut m = started();
    let end = T0 + 45.0 * MIN;
    m.handle(Tick, end);
    m.handle(Snooze, end);
    assert_eq!(m.pill_text(end + 15.0).as_deref(), Some("01:45"));
    assert_eq!(m.icon(), PillIcon::Snoozed);
}

#[test]
fn paused_title() {
    let mut m = started();
    m.handle(Pause, T0 + 60.0);
    assert_eq!(m.pill_text(T0 + 500.0).as_deref(), Some("44:00"));
}

#[test]
fn session_progress_basic() {
    let mut m = TimerMachine::default();
    assert_eq!(m.session_progress(), SessionProgress::Session { current: 1, of: 4 });
    m.handle(Start, T0);
    m.handle(Tick, T0 + 45.0 * MIN);
    assert_eq!(m.session_progress(), SessionProgress::Done { done: 1, of: 4, long_break_next: false });
    m.handle(Skip, T0 + 46.0 * MIN);
    assert_eq!(m.session_progress(), SessionProgress::Session { current: 2, of: 4 });
}

#[test]
fn session_progress_long_break_and_postponed() {
    let mut s = TimerSettings { sessions_per_set: 1, ..Default::default() };
    let mut m = TimerMachine::new(s.clone());
    m.handle(Start, T0);
    m.handle(Tick, T0 + 45.0 * MIN);
    assert_eq!(m.session_progress(), SessionProgress::LongBreak);

    s.delay_long_break = true;
    let mut d = TimerMachine::new(s);
    d.handle(Start, T0);
    d.handle(Tick, T0 + 45.0 * MIN);
    assert_eq!(d.session_progress(), SessionProgress::Done { done: 1, of: 1, long_break_next: true });
    d.handle(Skip, T0 + 46.0 * MIN);
    assert_eq!(d.session_progress(), SessionProgress::LongBreakAfterThisSession);
}

#[test]
fn available_actions() {
    let mut m = TimerMachine::default();
    assert_eq!(m.available(), AvailableActions::default());
    m.handle(Start, T0);
    assert!(m.available().can_pause);
    assert!(!m.available().can_skip);
    m.handle(Tick, T0 + 45.0 * MIN);
    let a = m.available();
    assert!(a.can_start_rest && a.can_snooze && a.can_skip);
    m.handle(StartRest, T0 + 46.0 * MIN);
    let a = m.available();
    assert!(a.can_extend && a.can_skip && a.can_pause);
}

#[test]
fn snooze_unavailable_at_limit() {
    let s = TimerSettings { max_snoozes_per_break: 1, ..Default::default() };
    let mut m = TimerMachine::new(s);
    m.handle(Start, T0);
    let mut now = T0 + 45.0 * MIN;
    m.handle(Tick, now);
    m.handle(Snooze, now);
    now += 121.0;
    m.handle(Tick, now);
    assert!(!m.available().can_snooze);
}

// CommandAndNameTests

#[test]
fn url_commands() {
    assert_eq!(UrlCommand::from_url("breakbar://startRest"), Some(UrlCommand::Event(StartRest)));
    assert_eq!(UrlCommand::from_url("BreakBar://TOGGLE"), Some(UrlCommand::Toggle));
    assert_eq!(UrlCommand::from_url("breakbar://startStop"), Some(UrlCommand::Toggle));
    assert_eq!(UrlCommand::from_url("breakbar://nonsense"), None);
    assert_eq!(UrlCommand::from_url("other://start"), None);
    // Windows may add a trailing slash.
    assert_eq!(UrlCommand::from_url("breakbar://pause/"), Some(UrlCommand::Event(Pause)));
}

#[test]
fn state_names() {
    assert_eq!(TimerState::Idle.name(), "idle");
    assert_eq!(TimerState::Resting { kind: RestKind::Long, ends_at: 0.0 }.name(), "restingLong");
}

#[test]
fn volumes_are_clamped_and_defaults_quiet() {
    let mut s = TimerSettings::default();
    assert_eq!(s.ticking_volume, 0.0);
    assert_eq!(s.windup_volume, 0.0);
    s.ding_volume = 7.0;
    s.ticking_volume = -1.0;
    assert_eq!(s.clamped().ding_volume, 1.0);
    assert_eq!(s.clamped().ticking_volume, 0.0);
}

// DotsTests

#[test]
fn dots_while_working_on_second_session() {
    let mut m = TimerMachine::default();
    m.handle(Start, T0);
    m.handle(Tick, T0 + 45.0 * MIN);
    m.handle(Skip, T0 + 46.0 * MIN);
    assert_eq!(m.dots(), DotProgress { done: 1, has_current: true, total: 4 });
}

#[test]
fn dots_while_waiting_to_rest() {
    let mut m = TimerMachine::default();
    m.handle(Start, T0);
    m.handle(Tick, T0 + 45.0 * MIN);
    assert_eq!(m.dots(), DotProgress { done: 1, has_current: false, total: 4 });
}

#[test]
fn dots_during_long_break_are_all_done() {
    let s = TimerSettings { sessions_per_set: 2, ..Default::default() };
    let mut m = TimerMachine::new(s);
    let mut now = T0;
    for _ in 0..2 {
        m.handle(Start, now);
        now += 45.0 * MIN;
        m.handle(Tick, now);
        if m.state() != TimerState::Idle && m.available().can_skip {
            m.handle(Skip, now);
        }
        m.handle(Stop, now);
    }
    assert_eq!(m.session_progress(), SessionProgress::Session { current: 1, of: 2 });
    assert_eq!(m.dots(), DotProgress { done: 0, has_current: true, total: 2 });
}

#[test]
fn dots_idle_at_start_of_set() {
    assert_eq!(TimerMachine::default().dots(), DotProgress { done: 0, has_current: true, total: 4 });
}

#[test]
fn dots_never_exceed_total() {
    let s = TimerSettings { sessions_per_set: 1, delay_long_break: true, ..Default::default() };
    let mut m = TimerMachine::new(s);
    m.handle(Start, T0);
    m.handle(Tick, T0 + 45.0 * MIN);
    let d = m.dots();
    assert!(d.done <= d.total);
}
