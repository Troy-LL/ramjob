# SDD progress — RamJob

Milestone: **M4** (Job Object backstop) — starting  
Prior: **M3 closed** on `milestone/m3-thermo-fix` after thermo + ticket 30.

## M3 closeout

| Item | Status |
|---|---|
| Design + plan + tray UI | done (merged PR #1 + parity) |
| Thermo CQ | done — [Thermo](b7af3b7f-0ef2-48ac-a9f9-6ddb18247688): Crit 2 / Imp 5 |
| Ticket 30 thermo fixes | done `afc0166` — review APPROVED |
| Lesson | `always-on-engine-cadence` (panel visibility ≠ stop engine) |

Verify: doctor ok; `cargo test --workspace` green; `ramjob-app` builds.

## M4 tasks

| Task | Status | Commit |
|---|---|---|
| 1 commit_ratio math | done | `ada36db` |
| 2 job_backstop store | done | `9115f64` |
| 3 FSM Backstop action | done | `9d939bd` |
| 4 Runtime wire-up | done | `a199c1b` |
| 5 Hog integration verify | pending | |
| 6 Panel warning + SPEC | pending | |
| 7 Thermo + lessons | pending | |

Design: `docs/superpowers/specs/2026-08-03-m4-job-object-backstop-design.md`  
Plan: `docs/superpowers/plans/2026-08-03-m4-job-object-backstop.md`
