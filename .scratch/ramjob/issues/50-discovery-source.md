# 50 — DiscoverySource + sweep backend

**Milestone:** M5 · Plan Task 1

## Goal
`DiscoverySource` trait + `SweepDiscovery` that diffs enumerate snapshots into Spawn/Exit events.

## Acceptance
- `DiscoveryEvent::{Spawn,Exit}` with pid + create_time
- Sweep backend unit-tested with fake lists
- `pub mod discovery` in ramjob-core

## Verify
`cargo test -p ramjob-core discovery`
