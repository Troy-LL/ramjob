# 05 — CLI enumerate → group → print GF

**Milestone:** M0

**What to build:** `ramjob list` (default when no args) enumerates, groups, prints one line per visible group: display name or key, member count, GF in human units (MiB/GiB). Sorted by GF descending.

**Blocked by:** 04 — Group Footprint accountant

**Status:** ready-for-agent

- [ ] `ramjob` / `ramjob list` prints groups with GF ≥ 50 MB
- [ ] Output stable enough for smoke (columns or `key\tmembers\tgf_bytes`)
- [ ] Integration test or `#[cfg(test)]` smoke that runs enumerate→group→footprint on the live machine and asserts ≥ 1 group OR documents empty with skip
- [ ] `--help` documents `list`

**Verify:** `cargo test --workspace; cargo run -p ramjob-cli -- list`

**Notes:** Milestone verify for M0 also includes manual glance that Brave/Spotify/VS Code (if installed) do not cross-merge. Record result in `.superpowers/sdd/m0-verify.md`.
