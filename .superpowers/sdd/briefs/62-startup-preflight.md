# Brief — ticket 62 startup preflight

## Ticket

`.scratch/ramjob/issues/62-startup-preflight.md`
Start after ticket 61 review APPROVED.

## Own

`crates/ramjob-core/src/preflight.rs` (+ `lib.rs` export). Optional thin app call later in 63/64 — prefer core-complete report API here.

## Spec

- SPEC §5.4 Environment preflight (startup, once)
- Design: `docs/superpowers/specs/2026-08-03-m6-shippable-design.md`
- Once per process; `PreflightReport` with pagefile, total RAM, ≥32 GB dormancy note, privilege notes
- Push to diagnostics ring if core owns one; else return report for app to push (document choice)

## Job

1. TDD + implement
2. Env: `. .\scripts\dev-env.ps1`; `$env:CARGO_TARGET_DIR="$env:USERPROFILE\ramjob-target"`
3. `cargo test -p ramjob-core`
4. **Commit approved:** `feat(m6): startup preflight (task 3)`
5. progress.md + `.superpowers/sdd/task-62-report.md`

## Boundaries

No tray Settings toggle, no HKCU changes, no thermo.

## Report

DONE | DONE_WITH_CONCERNS | BLOCKED
