//! The Rest and Start working windows, in the four styles. A window set is opened and closed by
//! `reconcile`, which the timer thread calls every tick: it is the port of the Mac app's presenter.
//! (Windows are never built inside a sync command: that deadlocks on Windows.)
use crate::{dots_of, now, progress_of, Dots, Progress, Shared};
use breakbar_core::{PromptWindowStyle, TimerMachine, TimerState};
use serde::Serialize;
use tauri::{AppHandle, Manager, Monitor, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Everything a Rest / Start working window needs to draw itself.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PromptSpec {
    /// "rest" (waiting for you), "resting" (countdown) or "work" (break is over)
    kind: &'static str,
    long_break: bool,
    progress: Progress,
    dots: Dots,
    ends_at: Option<f64>,
    total: Option<f64>,
    snooze_minutes: Option<i32>,
    extend_minutes: Option<i32>,
    can_skip: bool,
}

/// None when no window should be showing for this state.
fn spec_of(m: &TimerMachine) -> Option<PromptSpec> {
    let a = m.available();
    let s = &m.settings;
    let mut spec = PromptSpec {
        kind: "rest",
        long_break: false,
        progress: progress_of(m),
        dots: dots_of(m),
        ends_at: None,
        total: None,
        snooze_minutes: None,
        extend_minutes: None,
        can_skip: false,
    };
    match m.state() {
        TimerState::AwaitingRest { kind, .. } => {
            spec.long_break = kind == breakbar_core::RestKind::Long;
            spec.snooze_minutes = a.can_snooze.then_some(s.snooze_minutes);
            spec.can_skip = true;
        }
        TimerState::Resting { kind, ends_at } => {
            spec.kind = "resting";
            spec.long_break = kind == breakbar_core::RestKind::Long;
            spec.ends_at = Some(ends_at);
            spec.extend_minutes = a.can_extend.then_some(s.extend_minutes);
            spec.can_skip = true;
        }
        TimerState::AwaitingWork { kind, .. } => {
            spec.kind = "work";
            spec.long_break = kind == breakbar_core::RestKind::Long;
            spec.extend_minutes = a.can_extend.then_some(s.extend_minutes);
        }
        _ => return None,
    }
    Some(spec)
}

fn style_name(s: PromptWindowStyle) -> &'static str {
    match s {
        PromptWindowStyle::Small => "small",
        PromptWindowStyle::Maximised => "maximised",
        PromptWindowStyle::FullScreen => "fullScreen",
        PromptWindowStyle::Banner => "banner",
    }
}

#[derive(Default)]
pub struct PromptState {
    /// Should the window be up (when the state has one)? Closing it with Hide / Esc clears this.
    visible: bool,
    last_tag: Option<String>,
    rest_total: f64,
    last_ends_at: Option<f64>,
    open_style: Option<PromptWindowStyle>,
    spec: Option<PromptSpec>,
    preview: Option<(PromptWindowStyle, PromptSpec)>,
    preview_gen: u64,
}

const LIVE: &str = "prompt-live";
const PREVIEW: &str = "prompt-preview";

enum Action {
    Open(PromptWindowStyle),
    Close,
    Nothing,
}

/// Decides whether the window should be on screen, and opens, swaps or closes it.
///
/// Rules (as on the Mac): it appears when the timer starts waiting for you; after "Start resting"
/// it stays up as a countdown; snooze, pause, work and idle have no window; closing it never
/// changes the timer.
pub fn reconcile(app: &AppHandle) {
    let shared = app.state::<Shared>();
    let at = now();
    let (spec, style, tag, ends) = {
        let m = shared.machine.lock().unwrap();
        let tag = if m.state().name().starts_with("resting") { "resting" } else { m.state().name() };
        let ends = if let TimerState::Resting { ends_at, .. } = m.state() { Some(ends_at) } else { None };
        (spec_of(&m), m.settings.window_style, tag, ends)
    };

    let action = {
        let mut st = shared.prompt.lock().unwrap();
        if st.last_tag.as_deref() != Some(tag) {
            st.last_tag = Some(tag.to_string());
            match tag {
                "awaitingRest" | "awaitingWork" => st.visible = true, // newly waiting for you
                "resting" => {}                                        // keep whatever it was
                _ => st.visible = false,
            }
        }
        // The progress bar needs the whole length of this rest: remember it, and add each extension.
        match ends {
            Some(end) => {
                match st.last_ends_at {
                    Some(last) if end > last => st.rest_total += end - last,
                    Some(_) => {}
                    None => st.rest_total = (end - at).max(1.0),
                }
                st.last_ends_at = Some(end);
            }
            None => st.last_ends_at = None,
        }
        match spec {
            Some(mut sp) if st.visible => {
                if sp.kind == "resting" {
                    sp.total = Some(st.rest_total);
                }
                st.spec = Some(sp);
                if st.open_style == Some(style) {
                    Action::Nothing
                } else {
                    st.open_style = Some(style);
                    Action::Open(style)
                }
            }
            _ => {
                st.spec = None;
                if st.open_style.take().is_some() { Action::Close } else { Action::Nothing }
            }
        }
    }; // the lock is released before any window is touched

    match action {
        Action::Open(style) => {
            close_windows(app, LIVE);
            open_windows(app, style, LIVE);
        }
        Action::Close => close_windows(app, LIVE),
        Action::Nothing => {}
    }
    // The pages poll `get_prompt` every tick, so there is nothing to push here.
}

fn close_windows(app: &AppHandle, prefix: &str) {
    for (label, w) in app.webview_windows() {
        if label.starts_with(prefix) {
            let _ = w.destroy();
        }
    }
}

fn active_monitor(app: &AppHandle) -> Option<Monitor> {
    let monitors = app.available_monitors().ok()?;
    if let Ok(c) = app.cursor_position() {
        for m in &monitors {
            let (p, s) = (m.position(), m.size());
            if c.x >= p.x as f64 && c.x < (p.x + s.width as i32) as f64 && c.y >= p.y as f64 && c.y < (p.y + s.height as i32) as f64 {
                return Some(m.clone());
            }
        }
    }
    app.primary_monitor().ok().flatten().or_else(|| monitors.into_iter().next())
}

/// (x, y, width, height) in physical px for one window of `style` on `mon`.
fn frame_for(style: PromptWindowStyle, mon: &Monitor) -> (i32, i32, u32, u32) {
    let sc = mon.scale_factor();
    let px = |v: f64| (v * sc).round() as i32;
    let area = mon.work_area();
    let (ax, ay, aw, ah) = (area.position.x, area.position.y, area.size.width as i32, area.size.height as i32);
    match style {
        // The windows carry 12 px of transparent margin so the card's shadow is not cut off.
        PromptWindowStyle::Small => {
            let (w, h) = (px(404.0), px(354.0));
            (ax + (aw - w) / 2, ay + (ah - h) / 2, w as u32, h as u32)
        }
        PromptWindowStyle::Maximised => {
            let m = px(40.0);
            (ax + m, ay + m, (aw - 2 * m).max(200) as u32, (ah - 2 * m).max(200) as u32)
        }
        PromptWindowStyle::FullScreen => (mon.position().x, mon.position().y, mon.size().width, mon.size().height),
        PromptWindowStyle::Banner => {
            let (w, h) = (px(604.0), px(100.0));
            (ax + aw - w - px(4.0), ay, w as u32, h as u32)
        }
    }
}

fn open_windows(app: &AppHandle, style: PromptWindowStyle, prefix: &str) {
    let active = active_monitor(app);
    let mut monitors: Vec<Monitor> = Vec::new();
    if style == PromptWindowStyle::FullScreen {
        // Every display gets the overlay; the one you are working on first (it takes the keyboard).
        if let Some(a) = &active {
            monitors.push(a.clone());
        }
        for m in app.available_monitors().unwrap_or_default() {
            if active.as_ref().map(|a| a.position() != m.position()).unwrap_or(true) {
                monitors.push(m);
            }
        }
    } else if let Some(a) = active {
        monitors.push(a);
    }

    for (i, mon) in monitors.iter().enumerate() {
        let (x, y, w, h) = frame_for(style, mon);
        // Small window and banner are click-only: they never take the keyboard from what you are typing.
        let takes_keyboard = matches!(style, PromptWindowStyle::Maximised | PromptWindowStyle::FullScreen) && i == 0;
        let built = WebviewWindowBuilder::new(app, format!("{prefix}-{i}"), WebviewUrl::App("prompt.html".into()))
            .title("BreakBar")
            .decorations(false)
            .transparent(style != PromptWindowStyle::FullScreen)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .focused(takes_keyboard)
            .visible(false)
            .build();
        if let Ok(win) = built {
            let _ = win.set_size(PhysicalSize::new(w, h));
            let _ = win.set_position(PhysicalPosition::new(x, y));
            let _ = win.show();
            if takes_keyboard {
                let _ = win.set_focus();
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptPayload {
    style: &'static str,
    spec: PromptSpec,
    preview: bool,
}

/// What this prompt window should show right now (the page asks every tick). `None`: close yourself.
#[tauri::command]
pub async fn get_prompt(window: WebviewWindow, shared: State<'_, Shared>) -> Result<Option<PromptPayload>, String> {
    let st = shared.prompt.lock().unwrap();
    if window.label().starts_with(PREVIEW) {
        return Ok(st.preview.as_ref().map(|(style, spec)| PromptPayload { style: style_name(*style), spec: spec.clone(), preview: true }));
    }
    Ok(match (&st.spec, st.open_style) {
        (Some(spec), Some(style)) => Some(PromptPayload { style: style_name(style), spec: spec.clone(), preview: false }),
        _ => None,
    })
}

/// Close / Hide / Esc. Never changes the timer.
#[tauri::command]
pub async fn hide_prompt(app: AppHandle, window: WebviewWindow, shared: State<'_, Shared>) -> Result<(), String> {
    if window.label().starts_with(PREVIEW) {
        shared.prompt.lock().unwrap().preview = None;
        close_windows(&app, PREVIEW);
    } else {
        shared.prompt.lock().unwrap().visible = false;
        reconcile(&app);
    }
    Ok(())
}

/// Shows a style with sample data (Settings > Windows). Nothing here touches the timer.
#[tauri::command]
pub async fn preview_prompt(app: AppHandle, shared: State<'_, Shared>, style: String) -> Result<(), String> {
    let style = match style.as_str() {
        "small" => PromptWindowStyle::Small,
        "maximised" => PromptWindowStyle::Maximised,
        "fullScreen" => PromptWindowStyle::FullScreen,
        "banner" => PromptWindowStyle::Banner,
        _ => return Err("unknown style".into()),
    };
    let snooze = shared.machine.lock().unwrap().settings.snooze_minutes;
    let spec = PromptSpec {
        kind: "rest",
        long_break: false,
        progress: Progress { kind: "done", current: 0, done: 2, of: 4, long_break_next: false },
        dots: Dots { done: 2, has_current: false, total: 4 },
        ends_at: None,
        total: None,
        snooze_minutes: Some(snooze),
        extend_minutes: None,
        can_skip: true,
    };
    let generation = {
        let mut st = shared.prompt.lock().unwrap();
        st.preview = Some((style, spec));
        st.preview_gen += 1;
        st.preview_gen
    };
    close_windows(&app, PREVIEW);
    open_windows(&app, style, PREVIEW);
    // A preview must never linger, least of all the full-screen one.
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(12));
        let shared = handle.state::<Shared>();
        let mut st = shared.prompt.lock().unwrap();
        if st.preview_gen == generation {
            st.preview = None;
            drop(st);
            close_windows(&handle, PREVIEW);
        }
    });
    Ok(())
}

