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
- [ ] Default remains Off for fresh config

## Verify

Manual smoke notes in report; `cargo build -p ramjob-app` OK.
