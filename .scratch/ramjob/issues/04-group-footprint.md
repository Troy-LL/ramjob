# 04 — Group Footprint accountant

**Milestone:** M0

**What to build:** Compute Group Footprint (GF) = sum of private working set across group members + unique-shared placeholder (0 or last cached; rare QueryWorkingSetEx may be stubbed returning 0 for M0 with a clear TODO API).

**Blocked by:** 03 — Install-root grouping

**Status:** done

- [x] `accountant::group_footprint(&AppGroup) -> u64` bytes
- [x] Private WS sum has unit tests (synthetic members)
- [x] Unique-shared API exists (`unique_shared_ws`) defaulting to 0 for M0 without panicking
- [x] Groups below 50 MB GF can be filtered by a helper `is_visible(gf)`

**Verify:** `cargo test -p ramjob-core accountant`

**Notes:** Do not run QueryWorkingSetEx inside hot tests. Keep types in `ramjob-core`.
