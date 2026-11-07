// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod store;

use breakbar_core::{PillIcon, SessionProgress, TimerEvent, TimerMachine, TimerSettings, TimerState};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Logical size of the widget window when it is just the pill, and the width of the dropdown.
/// The dropdown's height follows its content (the page reports it).
const COLLAPSED: (f64, f64) = (170.0, 48.0);
const EXPANDED_WIDTH: f64 = 280.0;

struct Shared {
    machine: Mutex<TimerMachine>,
    /// Where the collapsed pill sits (physical px) while the dropdown is open.
    anchor: Mutex<Option<(i32, i32)>>,
    /// Cursor, window and anchor positions (physical px) at the moment a drag began.
    drag: Mutex<Option<((f64, f64), (i32, i32), Option<(i32, i32)>)>>,
    /// The Settings tab to show when its window opens.
    pending_tab: Mutex<Option<String>>,
    config_dir: PathBuf,
}

impl Shared {
    fn save_state(&self, m: &TimerMachine) {
        store::save_state(
            &self.config_dir,
            &store::SavedState {
                completed_sessions: m.completed_sessions(),
                long_break_postponed: m.long_break_postponed(),
                last_active: m.last_active(),
            },
        );
    }
}

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Dots {
    done: i32,
    has_current: bool,
    total: i32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Actions {
    can_pause: bool,
    can_resume: bool,
    can_skip: bool,
    can_extend: bool,
    can_snooze: bool,
    can_start_rest: bool,
    can_start_work: bool,
}

/// Everything the page needs to draw itself.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct View {
    icon: &'static str,
    text: Option<String>,
    state: &'static str,
    idle: bool,
    dots: Dots,
    progress: Progress,
    can_reset: bool,
    actions: Actions,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    /// "session", "done", "longBreak" or "longBreakAfter"
    kind: &'static str,
    current: i32,
    done: i32,
    of: i32,
    long_break_next: bool,
}

fn view_of(m: &TimerMachine, at: f64) -> View {
    let icon = match m.icon() {
        PillIcon::Idle => "idle",
        PillIcon::Working => "working",
        PillIcon::Paused => "paused",
        PillIcon::WaitingToRest => "waitingToRest",
        PillIcon::Snoozed => "snoozed",
        PillIcon::ShortRest => "shortRest",
        PillIcon::LongRest => "longRest",
        PillIcon::WaitingToWork => "waitingToWork",
    };
    let d = m.dots();
    let a = m.available();
    let progress = match m.session_progress() {
        SessionProgress::Session { current, of } => Progress { kind: "session", current, done: 0, of, long_break_next: false },
        SessionProgress::Done { done, of, long_break_next } => Progress { kind: "done", current: 0, done, of, long_break_next },
        SessionProgress::LongBreak => Progress { kind: "longBreak", current: 0, done: 0, of: 0, long_break_next: false },
        SessionProgress::LongBreakAfterThisSession => {
            Progress { kind: "longBreakAfter", current: 0, done: 0, of: 0, long_break_next: false }
        }
    };
    View {
        icon,
        text: m.pill_text(at),
        state: m.state().name(),
        idle: m.state() == TimerState::Idle,
        dots: Dots { done: d.done, has_current: d.has_current, total: d.total },
        progress,
        can_reset: m.completed_sessions() > 0 || m.long_break_postponed(),
        actions: Actions {
            can_pause: a.can_pause,
            can_resume: a.can_resume,
            can_skip: a.can_skip,
            can_extend: a.can_extend,
            can_snooze: a.can_snooze,
            can_start_rest: a.can_start_rest,
            can_start_work: a.can_start_work,
        },
    }
}

#[tauri::command]
fn get_view(shared: State<Shared>) -> View {
    view_of(&shared.machine.lock().unwrap(), now())
}

#[tauri::command]
fn send(app: AppHandle, shared: State<Shared>, event: String) -> View {
    let at = now();
    let view = {
        let mut m = shared.machine.lock().unwrap();
        let ev = match event.as_str() {
            "toggle" => {
                if m.state() == TimerState::Idle { TimerEvent::Start } else { TimerEvent::Stop }
            }
            "start" => TimerEvent::Start,
            "stop" => TimerEvent::Stop,
            "startRest" => TimerEvent::StartRest,
            "startWork" => TimerEvent::StartWork,
            "snooze" => TimerEvent::Snooze,
            "skip" => TimerEvent::Skip,
            "extend" => TimerEvent::Extend,
            "pause" => TimerEvent::Pause,
            "resume" => TimerEvent::Resume,
            "resetSet" => TimerEvent::ResetSet,
            _ => return view_of(&m, at),
        };
        // Sounds and notifications (the returned effects) arrive in step 4.
        let _effects = m.handle(ev, at);
        shared.save_state(&m);
        view_of(&m, at)
    };
    let _ = app.emit("view", view.clone());
    view
}

#[derive(Serialize)]
struct Flip {
    up: bool,
    left: bool,
}

/// Grows the window into the dropdown (or back). Near a screen edge it grows the other way,
/// and reports which way so the page can keep the pill in the same spot on screen.
/// `height` is the dropdown's content height in logical px; call again when it changes.
#[tauri::command]
fn set_expanded(window: WebviewWindow, shared: State<Shared>, expanded: bool, height: Option<f64>) -> Result<Flip, String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let px = |v: f64| (v * scale).round() as i32;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let mut anchor = shared.anchor.lock().unwrap();

    if !expanded {
        window.set_size(tauri::LogicalSize::new(COLLAPSED.0, COLLAPSED.1)).map_err(|e| e.to_string())?;
        if let Some((ax, ay)) = anchor.take() {
            window.set_position(PhysicalPosition::new(ax, ay)).map_err(|e| e.to_string())?;
        }
        return Ok(Flip { up: false, left: false });
    }

    let (ax, ay) = *anchor.get_or_insert((pos.x, pos.y));
    let h = height.unwrap_or(300.0).clamp(COLLAPSED.1, 900.0);
    let (cw, ch) = (px(COLLAPSED.0), px(COLLAPSED.1));
    let (ew, eh) = (px(EXPANDED_WIDTH), px(h));
    let (mut sx, mut sy) = (0, 0);
    if let Some(mon) = window.current_monitor().map_err(|e| e.to_string())? {
        let area = mon.work_area();
        let (right, bottom) = (area.position.x + area.size.width as i32, area.position.y + area.size.height as i32);
        if ax + ew > right {
            sx = -(ew - cw); // grow to the left, pill stays at the right edge
        }
        if ay + eh > bottom {
            sy = -(eh - ch); // grow upward, pill stays at the bottom edge
        }
    }
    window.set_size(tauri::LogicalSize::new(EXPANDED_WIDTH, h)).map_err(|e| e.to_string())?;
    window.set_position(PhysicalPosition::new(ax + sx, ay + sy)).map_err(|e| e.to_string())?;
    Ok(Flip { up: sy != 0, left: sx != 0 })
}

/// Dragging is done by hand: the window moves by exactly how far the cursor has moved since the
/// press, so it never trails behind even when the mouse starts fast.
#[tauri::command]
fn drag_begin(window: WebviewWindow, shared: State<Shared>) -> Result<(), String> {
    let c = window.cursor_position().map_err(|e| e.to_string())?;
    let p = window.outer_position().map_err(|e| e.to_string())?;
    let anchor = *shared.anchor.lock().unwrap();
    *shared.drag.lock().unwrap() = Some(((c.x, c.y), (p.x, p.y), anchor));
    Ok(())
}

#[tauri::command]
fn drag_move(window: WebviewWindow, shared: State<Shared>) -> Result<(), String> {
    let Some(((cx, cy), (px, py), start_anchor)) = *shared.drag.lock().unwrap() else { return Ok(()) };
    let c = window.cursor_position().map_err(|e| e.to_string())?;
    let (dx, dy) = ((c.x - cx).round() as i32, (c.y - cy).round() as i32);
    if let Some((ax, ay)) = start_anchor {
        *shared.anchor.lock().unwrap() = Some((ax + dx, ay + dy)); // dragged while open: the pill moved too
    }
    window.set_position(PhysicalPosition::new(px + dx, py + dy)).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings(shared: State<Shared>) -> TimerSettings {
    shared.machine.lock().unwrap().settings.clone()
}

/// Applies new settings (clamped), saves them, and tells every window. Returns what was kept.
#[tauri::command]
fn update_settings(app: AppHandle, shared: State<Shared>, settings: TimerSettings) -> TimerSettings {
    let clamped = settings.clamped();
    let view = {
        let mut m = shared.machine.lock().unwrap();
        m.settings = clamped.clone();
        view_of(&m, now())
    };
    store::save_settings(&shared.config_dir, &clamped);
    let _ = app.emit("settings", clamped.clone());
    let _ = app.emit("view", view);
    clamped
}

#[tauri::command]
fn open_settings(app: AppHandle, shared: State<Shared>, tab: Option<String>) -> Result<(), String> {
    open_settings_window(&app, &shared, tab)
}

fn open_settings_window(app: &AppHandle, shared: &Shared, tab: Option<String>) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("settings") {
        if let Some(t) = tab {
            let _ = app.emit("show-tab", t);
        }
        let _ = w.show();
        let _ = w.unminimize();
        return w.set_focus().map_err(|e| e.to_string());
    }
    *shared.pending_tab.lock().unwrap() = tab;
    WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("BreakBar Settings")
        .inner_size(500.0, 560.0)
        .resizable(false)
        .maximizable(false)
        .center()
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The tab asked for when the Settings window was opened (taken once).
#[tauri::command]
fn take_tab(shared: State<Shared>) -> Option<String> {
    shared.pending_tab.lock().unwrap().take()
}

#[derive(Serialize)]
struct AppInfo {
    version: &'static str,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo { version: env!("CARGO_PKG_VERSION") }
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

fn show_widget(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_always_on_top(true);
        let _ = w.set_focus();
    }
}

fn main() {
    let app = tauri::Builder::default()
        // A second launch (for example from the Start menu) just brings the widget back.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_widget(app)))
        .invoke_handler(tauri::generate_handler![
            get_view,
            send,
            set_expanded,
            drag_begin,
            drag_move,
            get_settings,
            update_settings,
            open_settings,
            take_tab,
            app_info,
            quit
        ])
        .setup(|app| {
            // Saved settings and set counter (defaults when there is nothing, or it is damaged).
            let config_dir = app.path().app_config_dir()?;
            let settings = store::load_settings(&config_dir);
            let saved = store::load_state(&config_dir);
            let completed = saved.completed_sessions.clamp(0, settings.sessions_per_set);
            let machine = TimerMachine::restored(settings, completed, saved.long_break_postponed, saved.last_active);
            app.manage(Shared {
                machine: Mutex::new(machine),
                anchor: Mutex::new(None),
                drag: Mutex::new(None),
                pending_tab: Mutex::new(None),
                config_dir,
            });

            // Tray icon: static, lives in the overflow area unless the user pins it.
            let show = MenuItem::with_id(app, "show", "Show BreakBar", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &settings, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().ok_or("missing app icon")?)
                .tooltip("BreakBar")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_widget(app),
                    "settings" => {
                        let shared = app.state::<Shared>();
                        let _ = open_settings_window(app, &shared, None);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        show_widget(tray.app_handle());
                    }
                })
                .build(app)?;

            // One tick a quarter second; the page redraws from the "view" event.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut ticks = 0u32;
                loop {
                    std::thread::sleep(Duration::from_millis(250));
                    let at = now();
                    let view = {
                        let shared = handle.state::<Shared>();
                        let mut m = shared.machine.lock().unwrap();
                        let before = (m.completed_sessions(), m.long_break_postponed());
                        let _effects = m.handle(TimerEvent::Tick, at);
                        // Save when the set changes, and every 10 s while running (keeps "last active" fresh).
                        ticks = ticks.wrapping_add(1);
                        let changed = before != (m.completed_sessions(), m.long_break_postponed());
                        if changed || (ticks % 40 == 0 && m.state() != TimerState::Idle) {
                            shared.save_state(&m);
                        }
                        view_of(&m, at)
                    };
                    let _ = handle.emit("view", view);
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building BreakBar");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            let shared = handle.state::<Shared>();
            let m = shared.machine.lock().unwrap();
            shared.save_state(&m);
        }
    });
}
