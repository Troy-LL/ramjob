# SDD progress — RamJob

Milestone: M3 (tray + panel + sliders)

M0/M1/M2 closed earlier. M3 design + plan + SDD landed via `milestone/m3-tray-ui` (merged PR #1); post-merge parity fixes on `main`.

## M3 tasks (from plan / merge)

| Task | Status | Notes |
|---|---|---|
| Scaffold ramjob-app | done | Tauri v2 |
| History + ceiling ticks | done | core + chart |
| Cap snap/floor + overall_limit | done | config |
| Panel snapshot IPC | done | |
| Tray + lazy panel | done | |
| Instrument shell / gauges | done | |
| First-run + diagnostics copy | done | |
| Parity closure (IPC, show-all, drag) | done | `837196a`+ |
| Verify notes | done | `.superpowers/sdd/m3-verify.md` |
| Thermo CQ | done | ticket 30 — C1–C2, I3–I7 |

## Verify (2026-08-03 autonomous)

- `verify-ramjob` doctor: cargo=ok, msvc_link=ok, ramjob=ok, ramjob_list=ok
- `cargo test --workspace`: green (core 87 + m2_integration + hog)
- Tray E2E: blocked on this host when SAC rejects unsigned Tauri build scripts (see parity design SAC note). Source parity committed; GUI proof deferred.

## Thermo

- Pending: once for M3 (post-verify)
