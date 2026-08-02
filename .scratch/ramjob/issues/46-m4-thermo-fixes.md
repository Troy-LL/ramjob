# 46 — M4 thermo Critical/Important fixes

**Milestone:** M4  
**Source:** [Finish M4 thermo](7eba0988-c4b0-4c1c-8dca-43134c2e9749) → `.superpowers/sdd/m4-thermo-review.md`

## Acceptance

### Critical
**C1** Delete temp `JobHandle` + `mem::forget` in `set_memory_limit` / `assign_pid`. Pass `&job.handle` like `clear_limit`. Unit test: apply/assign `Err` leaves store job still usable.

### Important
**I1** Extract: move runtime backstop tests out of `runtime.rs`; extract backstop helpers to sibling module so `runtime.rs` production stays well under 1k (target: prod file ≪ 400 lines if practical).

**I2** One shared mock `BackstopHooks` for tests; delete triplicate; unify unlimited detection via limit flags not `job_memory_limit > 0`.

**I3** Single helper for translate→ratchet→set_memory_limit used by arm and cap-decrease.

**I4** Replace `Option<Option<u64>>` with explicit `JobLimitState` enum (or equivalent).

**I5** Single backstop side-effect dispatch after FSM action/follow; run `track_cap_change` once per group iteration (no miss on rate-limit continue).

Nice-to-haves N1–N5 optional if cheap while in the file; not required.

## Verify
```
. .\scripts\dev-env.ps1
$env:CARGO_TARGET_DIR="$env:USERPROFILE\ramjob-target"
cargo test -p ramjob-core
cargo test -p ramjob-core --test m4_backstop
```

## Commit
`fix(m4): thermo C1 handle Drop + Important extract (task 7)`
