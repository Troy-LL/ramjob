# 63 Tray Settings autostart wire

**Milestone.** M6

**What to build.** Tray Settings enables an Autostart toggle that persists `config.autostart` and
syncs the HKCU Run key.

**Blocked by.** 60, 61

**Status.** ready-for-agent

## Acceptance criteria

- [ ] Settings menu item no longer a dead stub for autostart
- [ ] Toggle On → save config + enable Run; Off → save + disable Run
- [ ] Startup syncs Run key to match loaded `config.autostart`
- [ ] On config load/startup: call `prune_stale_groups` (ticket 60 Nit) before save-back if pruned
- [ ] When groups are observed in the app/runtime path, refresh `last_seen_unix` for those keys
- [ ] Default remains Off for fresh config

## Verify

Manual smoke notes in report; `cargo build -p ramjob-app` OK.
