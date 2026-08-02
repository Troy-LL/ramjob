# Brief — 42 FSM Backstop

Repo: `E:/Troy/Code/Side Projects/Ram` · Branch: `milestone/m4-job-backstop`  
Ticket: `.scratch/ramjob/issues/42-fsm-backstop.md`  
Depends: can run parallel with 41 (different files: fsm.rs only) IF 41 does not touch fsm — yes parallel OK with 41.

## Own
`crates/ramjob-core/src/fsm.rs` (+ tests) only.

## Locked
- Rename `RecordWouldBackstop` → `Backstop`
- Emit `Backstop` only when `always_enforce` after 3 ineffective trims in 60s
- Without opt-in: no Backstop action (keep soft-stop / LowYield path as today)

Commit: `feat(m4): FSM Backstop action when opted in (task 3)`  
Report: `.superpowers/sdd/task-42-report.md`
