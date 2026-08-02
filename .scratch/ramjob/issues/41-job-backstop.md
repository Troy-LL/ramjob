# 41 — job_backstop Job Object store

**Milestone:** M4 · Plan Task 2  
**Depends on:** 40

## Goal
`JobBackstopStore`: create job per group, set/clear `JobMemoryLimit`, assign PIDs, Drop without kill-on-close.

## Acceptance
- `KILL_ON_JOB_CLOSE` never set; `BREAKAWAY_OK` off
- Assign failure → degrade Result + diagnostic-friendly error
- Unit tests for limit packing / degrade path

## Verify
`cargo test -p ramjob-core job_backstop`
