# Brief — 55 budget CI

Repo: `E:/Troy/Code/Side Projects/Ram` · Branch: `milestone/m5-etw-budget`  
Ticket: `.scratch/ramjob/issues/55-budget-ci.md`  
SPEC §6 ceilings: idle WS < 12 MB target / 25 MB ceiling; idle CPU < 0.1% / 0.3%.

## Own
`crates/ramjob-core/src/budget.rs` (+ lib) and/or `tests/` + `m5-verify.md`. Prefer measuring current process after Runtime::new idle settle — not full tray if SAC blocks app.

## Job
1. Sample own working set via windows-rs
2. Unit/integration: after brief sleep with disarmed runtime, WS ≤ 25 MB OR mark `#[ignore]` with m5-verify explanation if environment can't measure
3. Commit: `test(m5): budget CI harness (task 6)`
4. progress task 6; task-55-report.md

Wait until ticket 54 is committed if you need Runtime; otherwise pure budget helpers OK in parallel after 54 lands — if 54 still dirty on runtime, wait.

No AskQuestion. Skip long skill reads.
