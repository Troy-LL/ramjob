# Tray panel

The tray app shows a 420×600 instrument panel (left-click tray or Open): system state pill, hero gauge, per-app gauges, pause-all, and copy-diagnostics. Enforcement in this process runs only while the panel window is shown.

## Sub-features

- `panel-open` reveals the main WebView window titled `RamJob`.
- `panel-state` shows Idle / Armed / Warning on `#state-pill`.
- `panel-pause` toggles Pause all via footer or tray menu.
- `panel-diag` copies the diagnostics ring to the clipboard.

## How to get to it (user POV)

- Run `cargo run -p ramjob-app` (or a packaged RamJob build).
- Left-click the tray icon, or right-click → Open.
- Use footer **Pause all** / **Copy diagnostics**, or tray **Pause all** / **Quit**.

## Driving it with verify-ramjob

Preconditions:

- Interactive Windows desktop session (not headless CI).
- Doctor OK for toolchain; separately `cargo build -p ramjob-app` succeeds.
- No second `ramjob-app` already owned by the user — if one is running, **stop and report unreachable** rather than attaching to it.

- **Launch.** Start `cargo run -p ramjob-app` in its own terminal; record the shell/cargo PID in `pids.txt` only if this verify run started it.
- **Open.** Left-click tray or choose Open so the window title is `RamJob`.
- **Observe.** `#state-pill` is visible; `#app-grid` lists apps or empty state; status text is populated.
- **Proof.** Screenshot of the panel with title/branding visible saved under `<evidence>\tray-panel.png`, plus a short `tray-panel.notes.txt` describing Idle/Armed and whether any app rows appeared. Headless agents: mark this feature **unreachable** with reason `no interactive desktop` — do not substitute `cargo test` as a pass for this file’s UI proof.
- **Cleanup.** Quit via tray Quit (or close only the process this run started). Do not wipe AppData config.

## Gotchas

- No `--config` on the app — it always uses `%APPDATA%\RamJob\config.toml`.
- No `--simulate-armed` for the panel; Armed/Warning may stay Idle without pressure.
- Building `ramjob-app` pulls Tauri deps; Smart App Control may block fresh build scripts.
- Dual-running CLI `ramjob run` and the panel causes conflicting enforcement stories.
