use crate::Time;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestKind {
    Short,
    Long,
}

/// The part of a timed phase that survives a pause.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimedPhase {
    Work,
    Rest(RestKind),
}

/// Where the timer is right now. Each variant carries only what it needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimerState {
    Idle,
    Working { ends_at: Time },
    /// Work finished; the Rest window is up, waiting for "Start resting".
    AwaitingRest { kind: RestKind, since: Time, snoozes: i32 },
    /// "Snooze" pressed; the window comes back at `until`.
    Snoozed { kind: RestKind, until: Time, snoozes: i32 },
    Resting { kind: RestKind, ends_at: Time },
    /// Rest finished; the Start working window is up.
    AwaitingWork { kind: RestKind, since: Time },
    Paused { phase: TimedPhase, remaining: f64, since: Time },
}

impl TimerState {
    /// Short stable name, used by the event log.
    pub fn name(&self) -> &'static str {
        match self {
            TimerState::Idle => "idle",
            TimerState::Working { .. } => "working",
            TimerState::AwaitingRest { .. } => "awaitingRest",
            TimerState::Snoozed { .. } => "snoozed",
            TimerState::Resting { kind: RestKind::Long, .. } => "restingLong",
            TimerState::Resting { .. } => "resting",
            TimerState::AwaitingWork { .. } => "awaitingWork",
            TimerState::Paused { .. } => "paused",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerEvent {
    Start,      // idle -> working
    Stop,       // any -> idle
    Tick,       // call about once a second
    StartRest,  // "Start resting" button
    StartWork,  // "Start working" button
    Snooze,
    Skip,       // skip the rest (or the wait) and go to work
    Extend,     // + extend_minutes
    Pause,
    Resume,
    ResetSet,
}

/// Things the app layer must do (sound, notification). Windows are driven by the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerEffect {
    PlayWindup,
    StartTicking,
    StopTicking,
    PlayDing,
    NotifyRestStarted(RestKind), // also sent when the Rest window appears
    NotifyRestEnded,
}
