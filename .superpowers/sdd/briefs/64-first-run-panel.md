# Brief — ticket 64 first-run + preflight panel

## Ticket

`.scratch/ramjob/issues/64-first-run-panel.md`
Start after 63 review APPROVED. Preflight already runs at startup (63).

## Own

`crates/ramjob-app` panel snapshot IPC + `ui/app.js` (and CSS if needed). Minimal core snapshot field additions if required.

## Spec

- SPEC §7.3 first-run (no wizard; top consumers + one-line explainer)
- §5.4 high-RAM dormancy note from preflight when applicable
- Design: `docs/superpowers/specs/2026-08-03-m6-shippable-design.md`
- Short copy; no new dashboard chrome

## Job

1. Snapshot fields: first_run (no caps), preflight note(s)
2. UI shows explainer + dormancy note
3. Env + `cargo build -p ramjob-app`; `cargo test -p ramjob-app` / core as needed
4. **Commit approved:** `feat(m6): first-run and preflight panel copy (task 5)`
5. progress.md + task-64-report.md

## Report

DONE | DONE_WITH_CONCERNS | BLOCKED
