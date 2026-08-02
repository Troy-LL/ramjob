# 52 — WMI discovery fallback + selector

**Milestone:** M5 · Plan Task 3 · Depends: 51

## Goal
`WmiProcessSource` + `select_discovery()`: ETW → WMI → Sweep with one-shot degrade diagnostic.

## Acceptance
- WMI backend implements DiscoverySource (or honest stub that Errs → sweep)
- Selector tries ETW, else WMI, else Sweep
- Records which mode was selected; emits etw_degrade_diagnostic once when falling back
- Tests for selector order with injectable failures

## Verify
`cargo test -p ramjob-core discovery`
