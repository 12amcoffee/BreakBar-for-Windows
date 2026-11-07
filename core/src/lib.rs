//! BreakBar's timer logic, shared by every part of the Windows app.
//!
//! This is the Rust twin of the Mac app's `BreakBarCore`. It has no UI, no threads and no clock:
//! the caller passes `now` to every call, which is what makes all of it testable. The rules,
//! the settings keys and the tests match the Mac app, so both apps behave the same way.

mod display;
mod machine;
mod settings;
mod state;
mod url_command;

/// A moment in time: seconds since the Unix epoch. The caller supplies it; nothing here reads a clock.
pub type Time = f64;

pub use display::{format_clock, AvailableActions, DotProgress, PillIcon, SessionProgress};
pub use machine::TimerMachine;
pub use settings::{PromptWindowStyle, TimerSettings, WhenRestEnds, WhenWorkEnds};
pub use state::{RestKind, TimedPhase, TimerEffect, TimerEvent, TimerState};
pub use url_command::UrlCommand;
