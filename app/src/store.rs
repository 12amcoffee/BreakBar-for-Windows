//! Saving settings and the set counter as small JSON files in the app's config folder.
//! Reading is forgiving: a missing, old or damaged file just means defaults.
use breakbar_core::{Time, TimerSettings};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct SavedState {
    pub completed_sessions: i32,
    pub long_break_postponed: bool,
    pub last_active: Option<Time>,
}

fn read<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// Writes to a temp file first, so a crash mid-write never leaves a half-written file.
fn write<T: Serialize>(dir: &Path, name: &str, value: &T) {
    let Ok(json) = serde_json::to_string_pretty(value) else { return };
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = dir.join(format!("{name}.tmp"));
    if fs::write(&tmp, json).is_ok() {
        let _ = fs::rename(&tmp, dir.join(name));
    }
}

pub fn load_settings(dir: &Path) -> TimerSettings {
    read::<TimerSettings>(&dir.join("settings.json")).unwrap_or_default().clamped()
}

pub fn save_settings(dir: &Path, s: &TimerSettings) {
    write(dir, "settings.json", s);
}

pub fn load_state(dir: &Path) -> SavedState {
    read(&dir.join("state.json")).unwrap_or_default()
}

pub fn save_state(dir: &Path, s: &SavedState) {
    write(dir, "state.json", s);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("breakbar-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn settings_round_trip_and_defaults() {
        let d = temp("settings");
        assert_eq!(load_settings(&d), TimerSettings::default());
        let s = TimerSettings { work_minutes: 50, ding_volume: 0.25, ..Default::default() };
        save_settings(&d, &s);
        assert_eq!(load_settings(&d), s);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn damaged_files_fall_back_to_defaults() {
        let d = temp("damaged");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("settings.json"), "{ not json").unwrap();
        fs::write(d.join("state.json"), "").unwrap();
        assert_eq!(load_settings(&d), TimerSettings::default());
        assert_eq!(load_state(&d), SavedState::default());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn state_round_trip() {
        let d = temp("state");
        let s = SavedState { completed_sessions: 3, long_break_postponed: true, last_active: Some(12.5) };
        save_state(&d, &s);
        assert_eq!(load_state(&d), s);
        let _ = fs::remove_dir_all(&d);
    }
}
