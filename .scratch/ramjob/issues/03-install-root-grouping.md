# 03 — Install-root grouping + identity

**Milestone:** M0

**What to build:** Group processes by install-root heuristic with version-segment stripping, runtime-host denylist (PPID walk), RamJob self-exclusion, and basic visibility filters from SPEC §5.2 (session ≠ 0, not under windir critical denylist stubs as listed in SPEC).

**Blocked by:** 02 — Process enumeration

**Status:** ready-for-agent

- [ ] `grouper::group_processes(&[ProcessRecord]) -> Vec<AppGroup>` with stable `group_key` (normalized install root)
- [ ] Version segments matching `^(app-)?v?\d+(\.\d+)+$` and `^current$` stripped from key
- [ ] Runtime hosts (`msedgewebview2`, `java`, `javaw`, `python`, `pythonw`, `node`, `dotnet`, `wscript`) join ancestor group or stay ungrouped
- [ ] Current process tree (self PID + descendants) excluded
- [ ] Unit tests with synthetic paths: Discord `app-1.0.x` strips; two apps under different roots never merge; runtime host under Brave joins Brave when PPID set

**Verify:** `cargo test -p ramjob-core grouper`

**Notes:** Signer/tree/image-name fallbacks can be stubs that assign a last-resort key from image name. Games filter: if path contains `steamapps\common`, mark `uncappable_game` but still group for M0 print (or exclude from display list — prefer exclude from displayed groups).
