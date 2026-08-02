# 56 — M5 thermo Critical/Important fixes

**Milestone:** M5  
**Source:** [Thermo CQ review M5](41e4b43a-24db-40ee-96cf-23b448325d08) → `.superpowers/sdd/m5-thermo-review.md`  
**Note:** `DiscoverySource: Send` already at `a13b22b`.

## Acceptance

### Critical
**C1** ETW: on open timeout/failure, stop session / close trace **before** join; never parent `GetLastError` for consumer OpenTraceW.
**C2** WMI: set `shutdown` before join on every failure path; never hang `select_discovery`.

### Important
**I1** ETW: delete `CALLBACK_QUEUE`; use logfile/event Context (`UserContext`) + `Arc::into_raw`/`from_raw`.
**I2** Sweep: one NtQSI/PathCache owner with Runtime (no double enumerate).
**I3** Test helpers: default to inert mock discovery; no live ETW/WMI in `new_with_backstop_store` / unit tests unless explicitly live.
**I4** Extract shared `QueuedDiscovery` (or equivalent) for ETW/WMI queue shell.
**I5** Store `DiscoveryMode` on Runtime.
**I6** WMI create_time / TZ alignment with PathCache identity (fix or document + test).

Nice-to-haves optional.

## Verify
```
. .\scripts\dev-env.ps1
$env:CARGO_TARGET_DIR="$env:USERPROFILE\ramjob-target"
cargo test -p ramjob-core
cargo build -p ramjob-cli -p ramjob-app
```

## Commit
`fix(m5): thermo C1/C2 lifecycle + Important discovery judo (task 7)`
