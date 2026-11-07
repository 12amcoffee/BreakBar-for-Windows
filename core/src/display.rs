use crate::{RestKind, Time, TimedPhase, TimerMachine, TimerState};

/// Which glyph the pill shows. The app maps these to icons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PillIcon {
    Idle,
    Working,
    Paused,
    WaitingToRest,
    Snoozed,
    ShortRest,
    LongRest,
    WaitingToWork,
}

/// What the "Session 2 of 4" line says, before translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionProgress {
    Session { current: i32, of: i32 },
    Done { done: i32, of: i32, long_break_next: bool },
    LongBreak,
    LongBreakAfterThisSession,
}

/// The row of session dots: `done` filled dots, then (if `has_current`) one ring for the session in progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DotProgress {
    pub done: i32,
    pub has_current: bool,
    pub total: i32,
}

/// Which buttons make sense right now. Shared by the dropdown and the Rest window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AvailableActions {
    pub can_pause: bool,
    pub can_resume: bool,
    pub can_skip: bool,
    pub can_extend: bool,
    pub can_snooze: bool,
    pub can_start_rest: bool,
    pub can_start_work: bool,
}

/// "07:05" style clock. Minutes are not wrapped (a 90 min session shows "90:00").
pub fn format_clock(seconds: i64) -> String {
    let s = seconds.max(0);
    format!("{:02}:{:02}", s / 60, s % 60)
}

impl TimerMachine {
    pub fn icon(&self) -> PillIcon {
        match self.state() {
            TimerState::Idle => PillIcon::Idle,
            TimerState::Working { .. } => PillIcon::Working,
            TimerState::Paused { .. } => PillIcon::Paused,
            TimerState::AwaitingRest { .. } => PillIcon::WaitingToRest,
            TimerState::Snoozed { .. } => PillIcon::Snoozed,
            TimerState::Resting { kind: RestKind::Long, .. } => PillIcon::LongRest,
            TimerState::Resting { .. } => PillIcon::ShortRest,
            TimerState::AwaitingWork { .. } => PillIcon::WaitingToWork,
        }
    }

    /// Text next to the pill's glyph, or `None` when idle. Countdowns round up, so a fresh 45 min
    /// session reads 45:00. Waiting states count *up* with a "+", so they differ from a running
    /// countdown. Snoozed and paused show a plain countdown; their glyphs say what state they are in.
    pub fn pill_text(&self, now: Time) -> Option<String> {
        let down = |end: Time| format_clock((end - now).ceil() as i64);
        let up = |start: Time| format!("+{}", format_clock((now - start) as i64));
        match self.state() {
            TimerState::Idle => None,
            TimerState::Working { ends_at } | TimerState::Resting { ends_at, .. } => Some(down(ends_at)),
            TimerState::AwaitingRest { since, .. } | TimerState::AwaitingWork { since, .. } => Some(up(since)),
            TimerState::Snoozed { until, .. } => Some(down(until)),
            TimerState::Paused { remaining, .. } => Some(format_clock(remaining.ceil() as i64)),
        }
    }

    /// Progress toward the next long break. The app turns this into translated text.
    pub fn session_progress(&self) -> SessionProgress {
        let n = self.settings.sessions_per_set;
        match self.state() {
            TimerState::Idle
            | TimerState::Working { .. }
            | TimerState::Paused { phase: TimedPhase::Work, .. } => {
                if self.long_break_postponed() {
                    SessionProgress::LongBreakAfterThisSession
                } else {
                    SessionProgress::Session { current: self.completed_sessions() + 1, of: n }
                }
            }
            TimerState::AwaitingRest { kind, .. }
            | TimerState::Snoozed { kind, .. }
            | TimerState::Resting { kind, .. }
            | TimerState::AwaitingWork { kind, .. }
            | TimerState::Paused { phase: TimedPhase::Rest(kind), .. } => {
                if kind == RestKind::Long {
                    SessionProgress::LongBreak
                } else {
                    SessionProgress::Done {
                        done: self.completed_sessions(),
                        of: n,
                        long_break_next: self.long_break_postponed(),
                    }
                }
            }
        }
    }

    /// Dots for the current set, e.g. 1 done + a ring + 2 empty while working on session 2 of 4.
    pub fn dots(&self) -> DotProgress {
        let n = self.settings.sessions_per_set;
        match self.session_progress() {
            SessionProgress::Session { current, .. } => {
                DotProgress { done: (current - 1).max(0).min(n), has_current: true, total: n }
            }
            SessionProgress::Done { done, .. } => {
                DotProgress { done: done.max(0).min(n), has_current: false, total: n }
            }
            SessionProgress::LongBreak | SessionProgress::LongBreakAfterThisSession => {
                DotProgress { done: n, has_current: false, total: n }
            }
        }
    }

    pub fn available(&self) -> AvailableActions {
        let s = &self.settings;
        let mut a = AvailableActions::default();
        match self.state() {
            TimerState::Working { .. } => {
                a.can_pause = s.pause_enabled;
            }
            TimerState::Resting { .. } => {
                a.can_pause = s.pause_enabled;
                a.can_skip = true;
                a.can_extend = s.extend_enabled;
            }
            TimerState::Paused { .. } => {
                a.can_resume = true;
            }
            TimerState::AwaitingRest { snoozes, .. } => {
                a.can_start_rest = true;
                a.can_skip = true;
                a.can_snooze = s.snooze_enabled && snoozes < s.max_snoozes_per_break;
            }
            TimerState::Snoozed { .. } => {
                a.can_start_rest = true;
                a.can_skip = true;
            }
            TimerState::AwaitingWork { .. } => {
                a.can_start_work = true;
                a.can_skip = true;
                a.can_extend = s.extend_enabled;
            }
            TimerState::Idle => {}
        }
        a
    }
}
