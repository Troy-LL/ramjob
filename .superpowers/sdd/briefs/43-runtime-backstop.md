# Brief — 43 Runtime Job Object wire-up

Repo: `E:/Troy/Code/Side Projects/Ram`  
Branch: `milestone/m4-job-backstop`  
Ticket: `.scratch/ramjob/issues/43-runtime-backstop.md`  
Depends: 40 (`commit_ratio`), 41 (`job_backstop`), 42 (`FsmAction::Backstop`) — all committed.

## Own
Primarily `crates/ramjob-core/src/runtime.rs`. May touch accountant/scanner helpers only if needed for `Σ PrivateUsage`. Tests in runtime or new test module.

## Job
1. Per-group `CommitRatio` map: sample during PRESSURE (`group commit / GF`).
2. On `FsmAction::Backstop` when opted in + `ready()`: assign member PIDs via `JobBackstopStore`, `set_memory_limit(translate_job_limit(C, ratio))`, diagnostic `BACKSTOP arm …`.
3. Assign failure → diagnostic degrade soft-only; do not panic.
4. On `SystemArm::Disarmed`: `clear_limit` all armed jobs; diagnostic `BACKSTOP disarm`.
5. Cap decrease while limited: use `ratchet_limit`.
6. Env + `cargo test -p ramjob-core` green (add focused tests with mock hooks).
7. Commit: `feat(m4): runtime arms Job Object backstop (task 4)`
8. Update progress.md; write `.superpowers/sdd/task-43-report.md`

Do not reinvent job/commit_ratio math. No AskQuestion. No panel UI (ticket 45).
