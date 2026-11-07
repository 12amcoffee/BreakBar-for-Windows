# BreakBar for Windows

A floating Pomodoro timer for Windows, the sibling of the [BreakBar menu bar app for Mac](https://github.com/12amcoffee/BreakBar).

**Status: work in progress** (step 4 of 6). A working floating timer with settings, sounds, notifications, hotkeys, start at login and `breakbar://` commands; the Rest windows come next. Test builds are attached to each pull request's CI run (unsigned).

## What is here

- `core/` is the timer: a pure Rust state machine (no UI, no clock, no sound), a port of the Mac app's Swift `Core` package. Settings are saved in the same JSON shape as on the Mac.
- `app/` is the Tauri app: the floating pill, its dropdown, the Settings window and the tray icon. Settings are saved in your app config folder (`%APPDATA%`).
- `core/tests/` holds the 59 tests from the Mac app, ported one for one, plus 2 checks on the settings JSON.

```bash
cargo test --workspace
```

## Plan

1. Timer logic (done)
2. Floating widget shell (Tauri, always on top, tray icon, single instance) (done)
3. Dropdown content and Settings (done)
4. Sounds, notifications, hotkeys, start at login, `breakbar://` commands (this step)
5. The four Rest window styles
6. Installer, release workflow, docs

## License

MIT, see [LICENSE](LICENSE). Inspiration is credited in [NOTICE](NOTICE).
