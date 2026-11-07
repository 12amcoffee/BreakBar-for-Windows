use serde::{Deserialize, Serialize};

/// What happens when a work session ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WhenWorkEnds {
    StartRestAutomatically,
    ShowRestWindow,
}

/// What happens when a rest ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WhenRestEnds {
    StartWorkAutomatically,
    ShowWorkWindow,
    StopTimer,
}

/// How a Rest / Start working prompt is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PromptWindowStyle {
    Small,
    Maximised,
    FullScreen,
    Banner,
}

/// Every user-configurable value that affects the timer logic.
///
/// The JSON keys match the Mac app's (`workMinutes`, `whenWorkEnds`, ...). A key that is missing
/// falls back to its default, so an older save, or a newer app, never makes the blob unreadable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TimerSettings {
    // Durations (minutes)
    pub work_minutes: i32,
    pub short_rest_minutes: i32,
    pub long_rest_minutes: i32,
    pub sessions_per_set: i32,

    // Transitions
    pub when_work_ends: WhenWorkEnds,
    pub when_rest_ends: WhenRestEnds,
    /// Work, rest, work with no prompts. Overrides both settings above.
    pub continuous_cycles: bool,

    // Optional controls
    pub pause_enabled: bool,
    pub extend_enabled: bool,
    pub extend_minutes: i32,
    pub snooze_enabled: bool,
    pub snooze_minutes: i32,
    pub max_snoozes_per_break: i32,

    // Long-break counter
    /// When a long break is due, take a short one now and the long one after the next session.
    pub delay_long_break: bool,
    /// Idle or unanswered longer than this starts a fresh set.
    pub inactivity_reset_minutes: i32,

    // Sounds (0 = off, 1 = full volume). Quiet by default: this app is meant for libraries.
    pub windup_volume: f64,
    pub ding_volume: f64,
    pub ticking_volume: f64,
    pub notifications_enabled: bool,

    // Presentation
    pub window_style: PromptWindowStyle,
}

impl Default for TimerSettings {
    fn default() -> Self {
        TimerSettings {
            work_minutes: 45,
            short_rest_minutes: 10,
            long_rest_minutes: 20,
            sessions_per_set: 4,
            when_work_ends: WhenWorkEnds::ShowRestWindow,
            when_rest_ends: WhenRestEnds::ShowWorkWindow,
            continuous_cycles: false,
            pause_enabled: true,
            extend_enabled: true,
            extend_minutes: 5,
            snooze_enabled: true,
            snooze_minutes: 2,
            max_snoozes_per_break: 3,
            delay_long_break: false,
            inactivity_reset_minutes: 60,
            windup_volume: 0.0,
            ding_volume: 0.5,
            ticking_volume: 0.0,
            notifications_enabled: true,
            window_style: PromptWindowStyle::Small,
        }
    }
}

impl TimerSettings {
    pub const WORK_RANGE: (i32, i32) = (1, 180);
    pub const REST_RANGE: (i32, i32) = (1, 180);
    pub const SET_RANGE: (i32, i32) = (1, 12);

    /// A copy with every value forced into a sane range.
    pub fn clamped(&self) -> TimerSettings {
        let mut s = self.clone();
        s.work_minutes = self.work_minutes.clamp(1, 180);
        s.short_rest_minutes = self.short_rest_minutes.clamp(1, 180);
        s.long_rest_minutes = self.long_rest_minutes.clamp(1, 180);
        s.sessions_per_set = self.sessions_per_set.clamp(1, 12);
        s.extend_minutes = self.extend_minutes.clamp(1, 60);
        s.snooze_minutes = self.snooze_minutes.clamp(1, 60);
        s.max_snoozes_per_break = self.max_snoozes_per_break.clamp(1, 20);
        s.inactivity_reset_minutes = self.inactivity_reset_minutes.clamp(5, 1440);
        s.windup_volume = clamp_volume(self.windup_volume);
        s.ding_volume = clamp_volume(self.ding_volume);
        s.ticking_volume = clamp_volume(self.ticking_volume);
        s
    }

    pub(crate) fn effective_when_work_ends(&self) -> WhenWorkEnds {
        if self.continuous_cycles {
            WhenWorkEnds::StartRestAutomatically
        } else {
            self.when_work_ends
        }
    }

    pub(crate) fn effective_when_rest_ends(&self) -> WhenRestEnds {
        if self.continuous_cycles {
            WhenRestEnds::StartWorkAutomatically
        } else {
            self.when_rest_ends
        }
    }
}

/// `f64::clamp` panics on NaN bounds only, but a NaN *value* would slip through, so map it to 0.
fn clamp_volume(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}
