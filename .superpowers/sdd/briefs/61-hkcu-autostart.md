# Brief — ticket 61 HKCU Run autostart

## Ticket

`.scratch/ramjob/issues/61-hkcu-autostart.md`
Blocked by 60 (DONE `540ca15`) — start after 60 review APPROVED unless controller says go.

## Own

New module under `crates/ramjob-core` (preferred) e.g. `autostart.rs`, exported from `lib.rs`. Do not wire tray yet (ticket 63).

## Spec

- Design: `docs/superpowers/specs/2026-08-03-m6-shippable-design.md`
- HKCU `Software\Microsoft\Windows\CurrentVersion\Run`, value name `RamJob`
- enable: quoted current exe path; disable: delete value; query: is_enabled
- Injectable registry trait for unit tests (no flaky real HKCU in CI if avoidable)

## Job

1. Implement + TDD
2. Env: `. .\scripts\dev-env.ps1`; `$env:CARGO_TARGET_DIR="$env:USERPROFILE\ramjob-target"`
3. `cargo test -p ramjob-core`
4. **Commit approved:** `feat(m6): HKCU Run autostart helper (task 2)`
5. Update progress.md; write `.superpowers/sdd/task-61-report.md`

## Report

DONE | DONE_WITH_CONCERNS | BLOCKED
