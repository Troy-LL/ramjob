# 11 — Synthetic memory hog harness binary

**Milestone:** M1

**What to build:** A `ramjob-hog` binary that allocates and touches RAM in scripted patterns: allocate-and-forget, allocate-and-loop, sawtooth. Used as ground truth for trim yield.

**Blocked by:** None — can start immediately (disjoint from enforcer after scaffold; serialize if sharing workspace Cargo.toml edits with 10).

**Status:** done

- [x] Workspace member `crates/ramjob-hog`
- [x] CLI flags: `--mode forget|loop|sawtooth`, `--mb <n>`, optional `--hold-secs`
- [x] Unit or smoke test that forget mode allocates without panic
- [x] Document how to run alongside `ramjob trim` / gate in README snippet in ticket report

**Verify:** `cargo test -p ramjob-hog; cargo run -p ramjob-hog -- --mode forget --mb 64 --hold-secs 1`

**Notes:** Keep dependency-light. No UI.
