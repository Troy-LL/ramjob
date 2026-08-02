# Brief — ticket 60 config autostart + prune

## Ticket

`.scratch/ramjob/issues/60-config-autostart-prune.md`

## Own

`crates/ramjob-core/src/config.rs` (+ tests in same file). Touch callers only if compile breaks from new struct fields (default them).

## Spec

- Design: `docs/superpowers/specs/2026-08-03-m6-shippable-design.md` — `autostart` default **false**
- SPEC §8.3 — pinned + 90-day prune; atomic save already exists
- Do not implement HKCU Run (ticket 61) or tray UI

## Job

1. Add `autostart: bool` (default false) to config types + TOML
2. Add `pinned: bool` and `last_seen_unix: u64` on groups (serde defaults)
3. Implement `prune_stale_groups` (90 days = 90 * 86400 seconds)
4. TDD unit tests; run `cargo test -p ramjob-core`
5. **Commit approved** this session: `feat(m6): config autostart + prune/pinned (task 1)`
6. Update `.superpowers/sdd/progress.md` task 1 → done with SHA
7. Write `.superpowers/sdd/task-60-report.md`

## Env

`. .\scripts\dev-env.ps1`; `$env:CARGO_TARGET_DIR="$env:USERPROFILE\ramjob-target"`

## Report

DONE | DONE_WITH_CONCERNS | BLOCKED + evidence.
