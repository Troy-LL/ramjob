# M3 tray-panel UI — final verification (Task 10)

Run at commit d2edb87 (base) + Task 10 changes. Environment: no GUI/display available to
this agent — commands were run, GUI/visual/drag items are marked manual.

## 1. `cargo test --workspace` green

**Verified — ran the command.** Output: all suites pass.
`ramjob_core`: 86 passed, 1 ignored, 0 failed. `m2_integration`: 1 passed.
`ramjob_hog` (lib + bin): 4 + 3 passed. `ramjob_app` (main.rs unit tests): 2 passed.
0 failures anywhere.

## 2. `cargo run -p ramjob-app` — tray + panel open

**Not runnable here** — this agent has no GUI/desktop session to launch and observe a
tray icon or panel window. Instead verified via:
- `cargo build -p ramjob-app` succeeds (clean build, ~1m36s, no warnings/errors).
- Code inspection: `crates/ramjob-app/src/main.rs` still registers the tray icon click
  handler and the lazy 420x600 panel window (Task 4/5 work) — untouched by Task 10, which
  only edited `crates/ramjob-app/ui/{index.html,styles.css,app.js}` and did not touch
  `main.rs`, `commands.rs`, or window-setup code.

**Manual verification needed**: actually launching the app and confirming the tray icon
appears and the panel opens at the right size/position.

## 3. Ceiling drag-release leaves vertical tick; does not Arm

Structurally verified in Task 8's review (chart drag handler in `app.js`
`renderHistoryChart`/`dragState`, commits via `onCommitLimit` -> `set_overall_limit`,
core-side `sys_history.rs::ceiling_commit_records_tick_without_sample` test, and
`policy.rs` arming logic never reacting to `overall_limit_bytes` changes).

Task 10 did not touch `renderHistoryChart`, `dragState`, `ceilingSegments`, or any core
policy/arming code — confirmed by diff: only additions were a first-run hint block and a
copy-diagnostics button/handler, both outside the chart and gauge code paths. Cited
verification stands unaffected.

## 4. Per-app marker persists to `%APPDATA%\RamJob\config.toml`

Structurally verified in Task 9's review (`panel.rs::set_cap` mutates
`self.config.groups[key].cap_bytes` then calls `save_config_atomic`; round-trip covered by
`panel::tests::set_cap_updates_existing_group` and `set_cap_applies_floor`, which reload
the file and assert the persisted value).

Confirmed untouched: Task 10 did not edit `panel.rs`, `config.rs`, or `commands.rs::set_cap`.
The app.js per-app gauge/marker rendering and drag-commit code (`callSetCap`,
`renderAppGrid`'s `onCommit`) is unchanged from Task 9.

**Manual verification needed**: confirming the on-disk file at
`%APPDATA%\RamJob\config.toml` actually updates after a real drag in the running app.

## 5. Armed pill turns blue under simulate-armed / Warning red with forced LowYield

`grep -rn "simulate.armed" crates` finds `--simulate-armed` implemented in
`crates/ramjob-cli/src/run.rs` (CLI-only flag, forces Armed for a tick, skipping OS dwell;
covered by its own test `run.rs::parses_forget_with_hold`-adjacent
`assert!(a.simulate_armed)`). **This is a CLI-only path** — there is no
`ramjob-app`/Tauri-side simulate-armed flag or command exposed to the panel UI, so there is
no way to trigger a live Armed/Warning transition end-to-end through the tray panel itself.
Noting this honestly as a **gap**, not fabricating a verification.

What IS verified: the pill/dot CSS and rendering logic itself
(`renderPill` in `app.js`: `.pill-armed` -> blue `#2f6fed`, `.pill-warning` -> red
`#c62828`, `.status-dot.armed`/`.warning` matching) was reviewed correct in Task 7's
review, and Task 10 did not touch `renderPill` or the pill CSS rules — confirmed by diff.
The core-side `warning` flag computation
(`panel::tests::warning_true_when_group_low_yield_or_thrashing`) is covered by an existing
green unit test using a forced `LowYield`/`Thrashing` fsm_hint fixture, which is the
snapshot-level equivalent of what the checklist asks for.

**Gap / manual verification needed**: no in-app way to force Armed/Warning live to see the
pill actually render blue/red on screen; only the mock snapshot in `app.js`
(`MOCK_SNAPSHOT.warning = false` currently) could be hand-edited for a browser-preview
check, which is a manual step for the user, not something this agent did.

## 6. Pause all stops trims (diagnostics show `pause_all`)

Built and tested in Task 3: `runtime.rs` line ~95-96 —
`if config.pause_all { self.diagnostics.push("pause_all".to_string()); }` — gates trims and
records the diagnostic string. Wired in Task 6 via `commands::pause_all` ->
`panel::set_pause_all` (`panel.rs:96`), tested by
`panel::tests::pause_toggles_flag_and_persists` and `status_line_paused`.

Confirmed untouched: Task 10 did not edit `runtime.rs` or `panel.rs`. In `app.js`,
`renderPauseButton` and the `pause-all-btn` click handler are unchanged; only a new
`copy-diagnostics-btn` listener was added alongside it in `main()`.

## 7. Panel ≤420×600; light theme

**Verified by reading config/CSS.**
`crates/ramjob-app/tauri.conf.json` lines 13-14: `"width": 420, "height": 600` (Task 4,
unchanged by Task 10).
`crates/ramjob-app/ui/styles.css`: light palette intact — `body { background: #e8eaed;
color: #1a1a1a; }`, white card/header backgrounds (`#fff`), light borders (`#d7d9dc`), blue
accent (`#2f6fed`), red warning (`#c62828`) — all values as set in Task 7, no dark-mode
media query, nothing overridden. Task 10 additions (`.first-run-hint`, `.link-btn.copied`)
use light-theme-consistent colors (`#eef4ff` bg / `#2f4d8f` text; `#1a7f37` green for the
copied state) and don't alter existing rules.

**Manual verification needed**: visually confirming the panel renders at ≤420x600 and
reads as "light theme" on an actual screen.

---

## Task 10 changes summary (for context on this file)

- `crates/ramjob-app/ui/index.html`: added `#first-run-hint` div under the info popover,
  and a `#copy-diagnostics-btn` button in the footer next to "Show all apps".
- `crates/ramjob-app/ui/app.js`: added `callCopyDiagnostics()` (invokes the existing
  `copy_diagnostics` Tauri command — no new Rust code), `renderFirstRunHint(snapshot,
  showAll)` (shows the hint only when no group has `cap_bytes > 0` AND the visible app
  count, after the existing 50MB floor filter, is ≤5), and a click handler on the copy
  button that calls `copy_diagnostics` then flashes "Copied ✓" for 1.5s.
- `crates/ramjob-app/ui/styles.css`: `.first-run-hint` (+ `.hidden`) and `.link-btn.copied`
  rules, light-theme-consistent.
- No Rust code was added or modified — `copy_diagnostics` (Task 6) and all core
  panel/runtime logic (Tasks 1-9) were reused as-is per the brief.

## Honest summary of verification method

- Ran directly: `cargo build -p ramjob-app`, `cargo test --workspace`, `node --check
  app.js`, and greps/reads confirming which files Task 10 touched vs. left alone.
- Cited rather than re-derived: Task 7 (pill CSS/logic), Task 8 (ceiling drag/tick), Task 9
  (per-app persistence), Task 3/6 (pause_all).
- Gaps found and stated plainly, not glossed over: no GUI session to launch the real app or
  see it visually; no panel-side simulate-armed control exists (only a CLI-only flag in
  ramjob-cli) so Warning/Armed pill colors can't be triggered end-to-end through the tray
  UI as written.

## Known limitations

- **Panel edits don't propagate to an already-running `ramjob run` CLI daemon.**
  `ramjob-app`'s tick loop (`run_tick`/`spawn_tick_loop` in `crates/ramjob-app/src/main.rs`)
  only enumerates/enforces while its panel window is visible — while the panel is closed,
  `ramjob-app` enforces nothing; `ramjob run` (`crates/ramjob-cli/src/run.rs`) is the
  always-on enforcement daemon in that case. However, `ramjob run` loads `config.toml` once
  at startup and never reloads it, so cap changes, `pause_all` toggles, or ceiling edits made
  through the panel do not reach a daemon that was already running before the edit. Today
  this means either: the user restarts the `ramjob run` process after editing config via the
  panel, or the user treats `ramjob-app` (panel/tray open) as the sole enforcement path and
  does not run `ramjob run` concurrently. Live config-reload for the CLI daemon is a real
  follow-up but is out of scope for this fix round.
