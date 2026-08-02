# 51 — ETW process discovery backend

**Milestone:** M5 · Plan Task 2 · Depends: 50

## Goal
`EtwProcessSource` for Kernel-Process; `Err` on open failure for fallback.

## Acceptance
- Attempts ETW subscription; returns Err if unavailable
- Maps start/stop to DiscoveryEvent Spawn/Exit when events available
- Unit/integration tests mock or ignore when ETW unavailable
- One-shot diagnostic string helper for degrade path

## Verify
`cargo test -p ramjob-core discovery`
