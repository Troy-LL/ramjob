# 10 — Soft trim pass

**Milestone:** M1

**What to build:** Soft-trim a group's processes via `EmptyWorkingSet` / `SetProcessWorkingSetSizeEx`, excluding foreground PID and visible top-level window owners. Global `trim_lock` so only one trim runs at a time. Member intersection by PID+create_time for ΔGF math.

**Blocked by:** None — can start immediately (M0 complete).

**Status:** ready-for-agent

- [ ] `enforcer::soft_trim_group(&AppGroup, &TrimContext) -> TrimOutcome` in ramjob-core
- [ ] Foreground exclusion per SPEC §4.2 / gaps
- [ ] Global trim lock (process-wide mutex)
- [ ] Unit tests with injectable hooks or mocked process list where possible; live test may be `#[ignore]` if privileges block
- [ ] Rate-limit constant: one trim per group per 20 s (API present even if CLI doesn't loop yet)

**Verify:** `cargo test -p ramjob-core enforcer`

**Notes:** Do not implement Job Object backstop (M4). No Tauri.
