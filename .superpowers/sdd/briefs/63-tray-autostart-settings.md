# Brief — ticket 63 tray Settings autostart

## Ticket

`.scratch/ramjob/issues/63-tray-autostart-settings.md`
Blockers 60–61 APPROVED. Preflight 62 APPROVED (`3f39944`) — may call `run_once`/`push_to_diagnostics` at startup if natural; panel copy is ticket 64.

## Own

`crates/ramjob-app` (main/tray/commands) + minimal core if needed for sync helpers.
Do not fight a parallel agent on `ramjob-core` beyond tiny compile fixes.

## Spec

- Design: `docs/superpowers/specs/2026-08-03-m6-shippable-design.md`
- Enable Settings → Autostart toggle; persist `config.autostart`; sync HKCU Run via `ramjob_core::autostart`
- Startup: sync Run to match config; `prune_stale_groups` on load + save-back if pruned; refresh `last_seen_unix` when groups observed
- Default autostart Off

## Job

1. Implement + verify `cargo build -p ramjob-app` and `cargo test -p ramjob-core`
2. Env: `. .\scripts\dev-env.ps1`; `$env:CARGO_TARGET_DIR="$env:USERPROFILE\ramjob-target"`
3. **Commit approved:** `feat(m6): tray autostart settings (task 4)`
4. progress.md + `.superpowers/sdd/task-63-report.md`

## Report

DONE | DONE_WITH_CONCERNS | BLOCKED
