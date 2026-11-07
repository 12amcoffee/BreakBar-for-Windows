# BreakBar for Windows

A floating Pomodoro timer for Windows, the sibling of the [BreakBar menu bar app for Mac](https://github.com/12amcoffee/BreakBar).

**Status: work in progress.** Only the timer logic exists so far (step 1 of 6). There is nothing to download yet.

## What is here

- `core/` is the timer: a pure Rust state machine (no UI, no clock, no sound), a port of the Mac app's Swift `Core` package. Settings are saved in the same JSON shape as on the Mac.
- `core/tests/` holds the 59 tests from the Mac app, ported one for one, plus 2 checks on the settings JSON.

```bash
cargo test --workspace
```

## Plan

1. Timer logic (this step)
2. Floating widget shell (Tauri, always on top, tray icon, single instance)
3. Dropdown content and Settings
4. Sounds, notifications, hotkeys, start at login, `breakbar://` commands
5. The four Rest window styles
6. Installer, release workflow, docs

## License

MIT, see [LICENSE](LICENSE). Inspiration is credited in [NOTICE](NOTICE).
