# 60 Config autostart + prune/pinned

**Milestone.** M6 (from SPEC.md §10)

**What to build.** Config.toml grows an `autostart` flag (default Off) and group entries honor
SPEC §8.3 `pinned` + 90-day prune so unused caps disappear on load without wiping user-pinned ones.

**Blocked by.** None.

**Status.** ready-for-agent

## Acceptance criteria

- [ ] `RamjobConfig.autostart: bool` defaults to `false`; parse + `save_config_atomic` round-trip
- [ ] `GroupConfig` has `pinned: bool` (default false) and `last_seen_unix: u64` (0 = unknown/legacy)
- [ ] `prune_stale_groups(cfg, now_unix)` drops groups with `!pinned` and `last_seen_unix > 0` older than 90 days; keeps pinned and never-seen (0) entries
- [ ] Existing configs without new fields still parse
- [ ] Unit tests cover defaults, round-trip, prune keep/drop

## Verify

`cargo test -p ramjob-core config` (or full `-p ramjob-core`) passes; new tests assert prune math.
