# Brief — 41 job_backstop

Repo: `E:/Troy/Code/Side Projects/Ram` · Branch: `milestone/m4-job-backstop`  
Ticket: `.scratch/ramjob/issues/41-job-backstop.md`  
Depends: ticket 40 committed.

## Own
`crates/ramjob-core/src/job_backstop.rs` + `lib.rs` only.

## Locked design
- `KILL_ON_JOB_CLOSE` off; `BREAKAWAY_OK` off
- Per-group job; set/clear JobMemoryLimit; assign PIDs
- Assign failure → `Err` (runtime degrades to soft-only)
- Drop closes handles; members must survive

Commit: `feat(m4): job_backstop Job Object store (task 2)`  
Report: `.superpowers/sdd/task-41-report.md`
