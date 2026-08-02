# Brief — ticket 65 M6 verify + ship notes

## Ticket

`.scratch/ramjob/issues/65-m6-verify-ship.md`
Start after 64 review APPROVED.

## Own

Docs only: `.superpowers/sdd/m6-verify.md`, README (or existing ship section). No product code unless a doc command is wrong and needs a one-line fix.

## Spec

- Design verify list: autostart toggle, preflight, first-run, tests
- Document `cargo build -p ramjob-app --release` + run path; no MSI claim
- SAC Off / `CARGO_TARGET_DIR` / `scripts/dev-env.ps1` notes if still relevant

## Job

1. Write accurate verify + ship notes from real commits on `milestone/m6-shippable`
2. **Commit approved:** `docs(m6): verify and ship notes (task 6)`
3. progress.md + task-65-report.md

## Report

DONE | DONE_WITH_CONCERNS | BLOCKED
