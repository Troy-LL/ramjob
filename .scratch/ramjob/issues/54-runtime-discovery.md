# 54 — Runtime consumes discovery deltas

**Milestone:** M5 · Plan Task 5 · Depends: 50–53

## Goal
Runtime applies DiscoveryEvent Spawn/Exit: invalidate PathCache on Exit; refresh hints on Spawn. Use select_discovery at startup; push degrade diagnostic once.

## Acceptance
- Runtime owns/holds DiscoverySource (or receives events each tick)
- Exit → path cache invalidate for pid+ctime
- Spawn → forces path resolve on next encounter (cache miss OK)
- Degrade diagnostic pushed once to DiagnosticsRing
- Unit tests with mock DiscoverySource

## Verify
`cargo test -p ramjob-core`
