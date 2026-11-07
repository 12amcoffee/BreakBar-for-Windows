// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use breakbar_core::{PillIcon, TimerEvent, TimerMachine, TimerState};
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindow};

/// Logical sizes of the widget window: the pill alone, and the pill with its dropdown.
const COLLAPSED: (f64, f64) = (170.0, 48.0);
const EXPANDED: (f64, f64) = (300.0, 340.0);

struct Shared {
    machine: Mutex<TimerMachine>,
    /// How far the window was shifted (physical px) to keep the dropdown on screen.
    shift: Mutex<(i32, i32)>,
    /// Cursor and window position (physical px) at the moment a drag began.
    drag: Mutex<Option<((f64, f64), (i32, i32))>>,
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
    actions: Actions,
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
    View {
        icon,
        text: m.pill_text(at),
        state: m.state().name(),
        idle: m.state() == TimerState::Idle,
        dots: Dots { done: d.done, has_current: d.has_current, total: d.total },
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
#[tauri::command]
fn set_expanded(window: WebviewWindow, shared: State<Shared>, expanded: bool) -> Result<Flip, String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let phys = |(w, h): (f64, f64)| ((w * scale).round() as i32, (h * scale).round() as i32);
    let (cw, ch) = phys(COLLAPSED);
    let (ew, eh) = phys(EXPANDED);
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let mut shift = shared.shift.lock().unwrap();

    if !expanded {
        window.set_size(tauri::LogicalSize::new(COLLAPSED.0, COLLAPSED.1)).map_err(|e| e.to_string())?;
        window
            .set_position(PhysicalPosition::new(pos.x - shift.0, pos.y - shift.1))
            .map_err(|e| e.to_string())?;
        *shift = (0, 0);
        return Ok(Flip { up: false, left: false });
    }

    let (mut sx, mut sy) = (0, 0);
    if let Some(mon) = window.current_monitor().map_err(|e| e.to_string())? {
        let area = mon.work_area();
        let (right, bottom) = (area.position.x + area.size.width as i32, area.position.y + area.size.height as i32);
        if pos.x + ew > right {
            sx = -(ew - cw); // grow to the left, pill stays at the right edge
        }
        if pos.y + eh > bottom {
            sy = -(eh - ch); // grow upward, pill stays at the bottom edge
        }
    }
    window.set_size(tauri::LogicalSize::new(EXPANDED.0, EXPANDED.1)).map_err(|e| e.to_string())?;
    window.set_position(PhysicalPosition::new(pos.x + sx, pos.y + sy)).map_err(|e| e.to_string())?;
    *shift = (sx, sy);
    Ok(Flip { up: sy != 0, left: sx != 0 })
}

/// Dragging is done by hand: the window moves by exactly how far the cursor has moved since the
/// press, so it never trails behind even when the mouse starts fast.
#[tauri::command]
fn drag_begin(window: WebviewWindow, shared: State<Shared>) -> Result<(), String> {
    let c = window.cursor_position().map_err(|e| e.to_string())?;
    let p = window.outer_position().map_err(|e| e.to_string())?;
    *shared.drag.lock().unwrap() = Some(((c.x, c.y), (p.x, p.y)));
    Ok(())
}

#[tauri::command]
fn drag_move(window: WebviewWindow, shared: State<Shared>) -> Result<(), String> {
    let Some(((cx, cy), (px, py))) = *shared.drag.lock().unwrap() else { return Ok(()) };
    let c = window.cursor_position().map_err(|e| e.to_string())?;
    let pos = PhysicalPosition::new(px + (c.x - cx).round() as i32, py + (c.y - cy).round() as i32);
    window.set_position(pos).map_err(|e| e.to_string())
}

fn show_widget(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_always_on_top(true);
        let _ = w.set_focus();
    }
}

fn main() {
    let machine = TimerMachine::default();

    tauri::Builder::default()
        // A second launch (for example from the Start menu) just brings the widget back.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_widget(app)))
        .manage(Shared { machine: Mutex::new(machine), shift: Mutex::new((0, 0)), drag: Mutex::new(None) })
        .invoke_handler(tauri::generate_handler![get_view, send, set_expanded, drag_begin, drag_move])
        .setup(|app| {
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
                        // The Settings screen arrives in step 3; for now this brings the widget back.
                        show_widget(app);
                        let _ = app.emit("open-settings", ());
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
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(250));
                let at = now();
                let view = {
                    let shared = handle.state::<Shared>();
                    let mut m = shared.machine.lock().unwrap();
                    let _effects = m.handle(TimerEvent::Tick, at);
                    view_of(&m, at)
                };
                let _ = handle.emit("view", view);
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running BreakBar");
}
