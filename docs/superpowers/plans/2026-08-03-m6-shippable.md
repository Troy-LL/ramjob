# M6 Shippable — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Autostart (HKCU Run), startup preflight, first-run polish — shippable v0.

**Architecture:** `preflight` + `autostart` modules in core/app; config `autostart` flag; tray Settings enables toggle; panel surfaces preflight/first-run copy.

**Tech Stack:** Rust, windows-rs registry, existing Tauri tray/panel.

## Global Constraints

- Design: `docs/superpowers/specs/2026-08-03-m6-shippable-design.md`
- Autostart **default Off** until user enables
- No MSI/service; HKCU Run only
- Battery / Chromium auto-backstop OPENs stay deferred
- Branch `milestone/m6-shippable`; one commit per task; poteto-agent + weaker Cursor model
- Do not start until M5 ticket 56 APPROVED

---

### Task 1: Config `autostart` + prune audit

- [ ] Add `autostart: bool` to RamjobConfig (default false); round-trip tests
- [ ] Confirm 90-day prune / pinned behavior or fill gaps
- [ ] Commit: `feat(m6): config autostart flag (task 1)`

### Task 2: HKCU Run autostart helper

- [ ] `autostart.rs`: enable/disable/query Run value for current exe path
- [ ] Unit tests with mock or safe registry under test key if needed
- [ ] Commit: `feat(m6): HKCU Run autostart helper (task 2)`

### Task 3: Preflight once at startup

- [ ] `preflight.rs`: pagefile, total RAM, SeDebug / privilege notes per SPEC §5.4
- [ ] Returns `PreflightReport`; diagnostics push
- [ ] Commit: `feat(m6): startup preflight (task 3)`

### Task 4: Wire app tray Settings + sync

- [ ] Enable Settings menu item → toggle autostart; persist config; sync Run key
- [ ] Commit: `feat(m6): tray autostart settings (task 4)`

### Task 5: Panel first-run + preflight copy

- [ ] Snapshot fields for first_run / high_ram dormancy note
- [ ] UI shows SPEC §7.3 one-liner + preflight note
- [ ] Commit: `feat(m6): first-run and preflight panel copy (task 5)`

### Task 6: Verify + ship notes

- [ ] `.superpowers/sdd/m6-verify.md` — autostart toggle, preflight, panel
- [ ] README quick start for release build
- [ ] Commit: `docs(m6): verify and ship notes (task 6)`

### Task 7: Thermo + lessons

- [ ] Thermo once; fix C/I; lesson capture; steering done
- [ ] Commit: `chore(m6): thermo fixes and lessons (task 7)`

---

## Success

User can build release app, enable autostart, see preflight honesty, set a cap — RamJob is a usable Windows RAM limiter.
