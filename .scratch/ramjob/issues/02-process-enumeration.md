# 02 — Process enumeration (NtQSI)

**Milestone:** M0

**What to build:** Enumerate processes via `NtQuerySystemInformation(SystemProcessInformation)` and expose each as a typed record with PID, PPID, session ID, image name, private working set, create time, and full image path when resolvable.

**Blocked by:** 01 — Rust workspace scaffold

**Status:** ready-for-agent

- [ ] `scanner::enumerate_processes() -> Vec<ProcessRecord>` implemented with windows-rs
- [ ] Unit/integration test covers at least the current process appearing with PID > 0 and private WS ≥ 0
- [ ] Full path cached by PID+create-time key (resolve once)
- [ ] Session 0 and missing-path processes still returned (filtering is later)

**Verify:** `cargo test -p ramjob-core scanner`

**Notes:** Prefer `windows` crate. Do not OpenProcess in a hot loop beyond path resolution. Exclude nothing yet except failing opens for path.
