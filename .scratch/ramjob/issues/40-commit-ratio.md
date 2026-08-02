# 40 — commit_ratio §3.2 math

**Milestone:** M4  
**Plan:** `docs/superpowers/plans/2026-08-03-m4-job-object-backstop.md` Task 1

## Goal

Pure `commit_ratio` module: EMA, sample count, translate/clamp/ratchet. No Win32.

## Acceptance

- `ready()` false until ≥3 samples
- `JobMemoryLimit = 1.15 × C × clamp(ratio, 1.0, 2.0)`
- Ratchet: `max(target, current_commit * 1.05)`
- Unit tests green

## Verify

`cargo test -p ramjob-core commit_ratio`
