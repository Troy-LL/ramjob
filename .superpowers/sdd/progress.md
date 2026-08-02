# SDD progress — RamJob

Milestone: **M5** (ETW + adaptive + budget CI) — starting  
Prior: **M4 closed** on `milestone/m4-job-backstop` after thermo + ticket 46 APPROVED (`7c1a0a0`).

## M4 closeout

| Item | Status |
|---|---|
| Tasks 1–6 | done |
| Thermo | Crit 1 / Imp 5 → fixed ticket 46 |
| Ticket 46 review | APPROVED |
| Lessons | `handle-forget-closes-job` |

## M5 tasks

| Task | Status | Commit |
|---|---|---|
| 1 DiscoverySource + sweep | done | feat(m5): DiscoverySource + sweep backend (task 1) |
| 2 ETW backend | done | feat(m5): ETW process discovery backend (task 2) |
| 3 WMI fallback | done | feat(m5): WMI discovery fallback (task 3) |
| 4 Adaptive sleep | done | feat(m5): adaptive polling ladder (task 4) |
| 5 Runtime discovery deltas | done | feat(m5): runtime applies discovery events (task 5) |
| 6 Budget CI harness | done | (this commit; run blocked by SAC 4551 — see m5-verify.md) |
| 7 Thermo + lessons | blocked | needs cargo test execution; SAC On |

Design: `docs/superpowers/specs/2026-08-03-m5-etw-adaptive-budget-design.md`  
Plan: `docs/superpowers/plans/2026-08-03-m5-etw-adaptive-budget.md`
