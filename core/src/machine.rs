use crate::{
    RestKind, TimedPhase, Time, TimerEffect, TimerEvent, TimerSettings, TimerState, WhenRestEnds,
    WhenWorkEnds,
};

/// The whole timer as one value. Feed it events, get back effects.
/// No timers, no UI: the caller supplies `now`, which makes it trivial to test.
#[derive(Clone, Debug, PartialEq)]
pub struct TimerMachine {
    state: TimerState,
    /// Work sessions completed in the current set (0 up to sessions_per_set).
    completed_sessions: i32,
    /// True once a long break has been postponed by "delay long break".
    long_break_postponed: bool,
    /// Last time the user was "around": used for the inactivity reset.
    last_active: Option<Time>,
    pub settings: TimerSettings,
}

impl Default for TimerMachine {
    fn default() -> Self {
        TimerMachine::new(TimerSettings::default())
    }
}

impl TimerMachine {
    /// A tick later than this after the end time means the PC slept: give up, don't advance.
    pub const OVERRUN_LIMIT: f64 = 60.0;

    pub fn new(settings: TimerSettings) -> Self {
        TimerMachine::restored(settings, 0, false, None)
    }

    /// A machine with the set counter restored from the last run.
    pub fn restored(
        settings: TimerSettings,
        completed_sessions: i32,
        long_break_postponed: bool,
        last_active: Option<Time>,
    ) -> Self {
        TimerMachine {
            state: TimerState::Idle,
            completed_sessions,
            long_break_postponed,
            last_active,
            settings: settings.clamped(),
        }
    }

    pub fn state(&self) -> TimerState {
        self.state
    }
    pub fn completed_sessions(&self) -> i32 {
        self.completed_sessions
    }
    pub fn long_break_postponed(&self) -> bool {
        self.long_break_postponed
    }
    pub fn last_active(&self) -> Option<Time> {
        self.last_active
    }

    // MARK: Entry point

    pub fn handle(&mut self, event: TimerEvent, now: Time) -> Vec<TimerEffect> {
        let was_idle = self.state == TimerState::Idle;

        let fx = match event {
            TimerEvent::Start => self.on_start(now),
            TimerEvent::Stop => self.go_idle(),
            TimerEvent::Tick => self.on_tick(now),
            TimerEvent::StartRest => self.on_start_rest(now),
            TimerEvent::StartWork => self.on_start_work(now),
            TimerEvent::Snooze => self.on_snooze(now),
            TimerEvent::Skip => self.on_skip(now),
            TimerEvent::Extend => self.on_extend(now),
            TimerEvent::Pause => self.on_pause(now),
            TimerEvent::Resume => self.on_resume(now),
            TimerEvent::ResetSet => {
                self.reset_set();
                Vec::new()
            }
        };

        if !(was_idle && self.state == TimerState::Idle) {
            self.last_active = Some(now);
        }
        fx
    }

    pub fn reset_set(&mut self) {
        self.completed_sessions = 0;
        self.long_break_postponed = false;
    }

    // MARK: Transitions

    fn on_start(&mut self, now: Time) -> Vec<TimerEffect> {
        if self.state != TimerState::Idle {
            return Vec::new();
        }
        if self.inactive_too_long(self.last_active, now) {
            self.reset_set();
        }
        self.begin_work(now)
    }

    fn go_idle(&mut self) -> Vec<TimerEffect> {
        if self.state == TimerState::Idle {
            return Vec::new();
        }
        let was_working = matches!(self.state, TimerState::Working { .. });
        self.state = TimerState::Idle;
        if was_working {
            vec![TimerEffect::StopTicking]
        } else {
            Vec::new()
        }
    }

    fn on_tick(&mut self, now: Time) -> Vec<TimerEffect> {
        match self.state {
            TimerState::Idle => Vec::new(),

            TimerState::Working { ends_at } => {
                if now < ends_at {
                    return Vec::new();
                }
                if now - ends_at > Self::OVERRUN_LIMIT {
                    return self.go_idle(); // slept: not counted
                }
                self.finish_work(now)
            }

            TimerState::Resting { ends_at, .. } => {
                if now < ends_at {
                    return Vec::new();
                }
                if now - ends_at > Self::OVERRUN_LIMIT {
                    return self.go_idle();
                }
                self.finish_rest(now)
            }

            TimerState::Snoozed { kind, until, snoozes } => {
                if self.inactive_too_long(Some(until - self.snooze_seconds()), now) {
                    return self.abandon();
                }
                if now >= until {
                    self.state = TimerState::AwaitingRest { kind, since: now, snoozes };
                }
                Vec::new()
            }

            TimerState::AwaitingRest { since, .. }
            | TimerState::AwaitingWork { since, .. }
            | TimerState::Paused { since, .. } => {
                if self.inactive_too_long(Some(since), now) {
                    self.abandon()
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn on_start_rest(&mut self, now: Time) -> Vec<TimerEffect> {
        match self.state {
            TimerState::AwaitingRest { kind, .. } | TimerState::Snoozed { kind, .. } => {
                self.state = TimerState::Resting { kind, ends_at: now + self.rest_seconds(kind) };
            }
            _ => {}
        }
        Vec::new()
    }

    fn on_start_work(&mut self, now: Time) -> Vec<TimerEffect> {
        match self.state {
            TimerState::AwaitingWork { .. } => self.begin_work(now),
            _ => Vec::new(),
        }
    }

    fn on_snooze(&mut self, now: Time) -> Vec<TimerEffect> {
        if !self.settings.snooze_enabled {
            return Vec::new();
        }
        if let TimerState::AwaitingRest { kind, snoozes, .. } = self.state {
            if snoozes < self.settings.max_snoozes_per_break {
                self.state = TimerState::Snoozed {
                    kind,
                    until: now + self.snooze_seconds(),
                    snoozes: snoozes + 1,
                };
            }
        }
        Vec::new()
    }

    /// Skip never touches the session counter (a skipped long break was already reset when it was chosen).
    fn on_skip(&mut self, now: Time) -> Vec<TimerEffect> {
        match self.state {
            TimerState::AwaitingRest { .. }
            | TimerState::Snoozed { .. }
            | TimerState::Resting { .. }
            | TimerState::AwaitingWork { .. } => self.begin_work(now),
            _ => Vec::new(),
        }
    }

    fn on_extend(&mut self, now: Time) -> Vec<TimerEffect> {
        if !self.settings.extend_enabled {
            return Vec::new();
        }
        let extra = f64::from(self.settings.extend_minutes * 60);
        match self.state {
            TimerState::Resting { kind, ends_at } => {
                self.state = TimerState::Resting { kind, ends_at: ends_at + extra };
            }
            TimerState::AwaitingWork { kind, .. } => {
                self.state = TimerState::Resting { kind, ends_at: now + extra };
            }
            _ => {}
        }
        Vec::new()
    }

    fn on_pause(&mut self, now: Time) -> Vec<TimerEffect> {
        if !self.settings.pause_enabled {
            return Vec::new();
        }
        match self.state {
            TimerState::Working { ends_at } => {
                self.state = TimerState::Paused {
                    phase: TimedPhase::Work,
                    remaining: (ends_at - now).max(0.0),
                    since: now,
                };
                vec![TimerEffect::StopTicking]
            }
            TimerState::Resting { kind, ends_at } => {
                self.state = TimerState::Paused {
                    phase: TimedPhase::Rest(kind),
                    remaining: (ends_at - now).max(0.0),
                    since: now,
                };
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn on_resume(&mut self, now: Time) -> Vec<TimerEffect> {
        let TimerState::Paused { phase, remaining, .. } = self.state else {
            return Vec::new();
        };
        match phase {
            TimedPhase::Work => {
                self.state = TimerState::Working { ends_at: now + remaining };
                vec![TimerEffect::StartTicking]
            }
            TimedPhase::Rest(kind) => {
                self.state = TimerState::Resting { kind, ends_at: now + remaining };
                Vec::new()
            }
        }
    }

    // MARK: Helpers

    fn begin_work(&mut self, now: Time) -> Vec<TimerEffect> {
        self.state = TimerState::Working { ends_at: now + f64::from(self.settings.work_minutes * 60) };
        vec![TimerEffect::PlayWindup, TimerEffect::StartTicking]
    }

    fn finish_work(&mut self, now: Time) -> Vec<TimerEffect> {
        self.completed_sessions += 1;
        let kind = self.choose_rest_kind();
        match self.settings.effective_when_work_ends() {
            WhenWorkEnds::StartRestAutomatically => {
                self.state = TimerState::Resting { kind, ends_at: now + self.rest_seconds(kind) };
            }
            WhenWorkEnds::ShowRestWindow => {
                self.state = TimerState::AwaitingRest { kind, since: now, snoozes: 0 };
            }
        }
        vec![TimerEffect::StopTicking, TimerEffect::PlayDing, TimerEffect::NotifyRestStarted(kind)]
    }

    fn finish_rest(&mut self, now: Time) -> Vec<TimerEffect> {
        let TimerState::Resting { kind, .. } = self.state else {
            return Vec::new();
        };
        let mut fx = vec![TimerEffect::NotifyRestEnded];
        match self.settings.effective_when_rest_ends() {
            WhenRestEnds::StartWorkAutomatically => fx.extend(self.begin_work(now)),
            WhenRestEnds::ShowWorkWindow => self.state = TimerState::AwaitingWork { kind, since: now },
            WhenRestEnds::StopTimer => self.state = TimerState::Idle,
        }
        fx
    }

    /// Decides short or long when work ends, and updates the counter.
    fn choose_rest_kind(&mut self) -> RestKind {
        let due = self.completed_sessions >= self.settings.sessions_per_set;
        if due && self.settings.delay_long_break && !self.long_break_postponed {
            self.long_break_postponed = true;
            return RestKind::Short;
        }
        if due {
            self.completed_sessions = 0; // long break starts: the set is over (also if it is skipped)
            self.long_break_postponed = false;
            return RestKind::Long;
        }
        RestKind::Short
    }

    /// The user walked away from a prompt: stop and start a fresh set.
    fn abandon(&mut self) -> Vec<TimerEffect> {
        self.reset_set();
        self.go_idle()
    }

    fn inactive_too_long(&self, since: Option<Time>, now: Time) -> bool {
        match since {
            None => false,
            Some(since) => now - since > f64::from(self.settings.inactivity_reset_minutes * 60),
        }
    }

    fn snooze_seconds(&self) -> f64 {
        f64::from(self.settings.snooze_minutes * 60)
    }

    fn rest_seconds(&self, kind: RestKind) -> f64 {
        let minutes = match kind {
            RestKind::Long => self.settings.long_rest_minutes,
            RestKind::Short => self.settings.short_rest_minutes,
        };
        f64::from(minutes * 60)
    }
}
