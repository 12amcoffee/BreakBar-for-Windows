use crate::TimerEvent;

/// Commands accepted by the `breakbar://` URL scheme, e.g. `breakbar://startRest`.
/// Windows hands the URL to the app as its first command-line argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlCommand {
    Event(TimerEvent),
    /// Start when idle, otherwise stop.
    Toggle,
}

impl UrlCommand {
    /// Case-insensitive. `None` for unknown commands.
    pub fn from_host(host: &str) -> Option<UrlCommand> {
        let event = match host.to_ascii_lowercase().as_str() {
            "start" => TimerEvent::Start,
            "stop" => TimerEvent::Stop,
            "toggle" | "startstop" => return Some(UrlCommand::Toggle),
            "startrest" => TimerEvent::StartRest,
            "startwork" => TimerEvent::StartWork,
            "snooze" => TimerEvent::Snooze,
            "skip" => TimerEvent::Skip,
            "extend" => TimerEvent::Extend,
            "pause" => TimerEvent::Pause,
            "resume" => TimerEvent::Resume,
            "resetset" => TimerEvent::ResetSet,
            _ => return None,
        };
        Some(UrlCommand::Event(event))
    }

    /// Parses `breakbar://<command>` (any letter case, an optional trailing slash, query or fragment).
    pub fn from_url(url: &str) -> Option<UrlCommand> {
        let (scheme, rest) = url.trim().split_once("://")?;
        if !scheme.eq_ignore_ascii_case("breakbar") {
            return None;
        }
        let host = rest.split(|c| c == '/' || c == '?' || c == '#').next().unwrap_or("");
        if host.is_empty() {
            return None;
        }
        UrlCommand::from_host(host)
    }
}
