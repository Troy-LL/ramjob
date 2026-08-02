# 53 — Adaptive polling ladder

**Milestone:** M5 · Plan Task 4

## Goal
Pure `next_sleep(arm, hottest_phase, panel_open) -> Duration` per SPEC §6.1; wire CLI + app tick loops.

## Acceptance
- Panel open → 1s
- Panel closed + Armed → based on max group phase (IDLE 15s / PRESSURE 3s / TRIM|BACKSTOP 1s) or full sweep 30s for discovery — use design: per-group ladder for enforcement cadence; app/CLI sleep = min of applicable intervals
- Panel closed + Disarmed → 120s (full sweep)
- Unit tests for table
- `ramjob run` and `ramjob-app` spawn_tick_loop use helper

## Verify
`cargo test -p ramjob-core adaptive`; `cargo build -p ramjob-cli -p ramjob-app`
