# 43 — Runtime Job Object wire-up

**Milestone:** M4 · Plan Task 4  
**Depends on:** 40, 41, 42

## Goal
PRESSURE samples commit_ratio; Backstop action arms job; DISARM clears limits; cap-decrease ratchet.

## Verify
`cargo test -p ramjob-core` + runtime-focused tests
