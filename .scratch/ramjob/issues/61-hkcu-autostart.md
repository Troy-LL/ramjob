# 61 HKCU Run autostart helper

**Milestone.** M6

**What to build.** Core/app helper enables, disables, and queries `HKCU\...\Run` for RamJob’s
current executable path so Settings can turn autostart on/off without a service.

**Blocked by.** 60

**Status.** ready-for-agent

## Acceptance criteria

- [ ] `autostart::{is_enabled, enable, disable}` (or equivalent) against Run key value name `RamJob`
- [ ] enable writes quoted path of current exe; disable deletes value
- [ ] Tests cover logic with injectable registry backend or documented safe test key
- [ ] No elevation required (HKCU only)

## Verify

Unit tests pass; manual note in report for Run key shape.
