# SDD progress — RamJob

Milestone: **M6** (Config, autostart, preflight, first-run) — thermo in progress  
Prior: **M5 closed** on `milestone/m5-etw-budget` after thermo + ticket 56 APPROVED (`6f1b279`).

## M5 closeout

| Item | Status |
|---|---|
| Tasks 1–7 | done |
| Thermo | Crit 2 / Imp 6 → fixed ticket 56 |
| Ticket 56 review | APPROVED (Spec PASS, Quality APPROVED) |
| Lessons | `etw-stop-before-join` (+ SAC execute-block note in `windows-smart-app-control-cargo`) |
| Verify | 151 tests; cli+app build; SAC Off on host |

## M6 tasks

| Task | Status | Commit |
|---|---|---|
| 1 Config autostart + prune/pinned | done | `540ca15` |
| 2 HKCU Run autostart helper | done | `a49f30b`, fix `91465f1` |
| 3 Startup preflight | done | `3f39944` |
| 4 Tray Settings autostart wire | done | `9307aee` |
| 5 First-run + preflight panel copy | done | `4aa0114` |
| 6 Verify + ship notes | done | `bde1257` |
| 7 Thermo + lessons | done | `4161e1e` |

Design: `docs/superpowers/specs/2026-08-03-m6-shippable-design.md`  
Plan: `docs/superpowers/plans/2026-08-03-m6-shippable.md`
