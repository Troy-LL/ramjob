# Brief — 40 commit_ratio

Repo: `E:/Troy/Code/Side Projects/Ram`  
Branch: create/use `milestone/m4-job-backstop` from current HEAD (includes M3 thermo fix).  
Ticket: `.scratch/ramjob/issues/40-commit-ratio.md`  
Plan Task 1: `docs/superpowers/plans/2026-08-03-m4-job-object-backstop.md`  
Design: `docs/superpowers/specs/2026-08-03-m4-job-object-backstop-design.md`

## Own
`crates/ramjob-core/src/commit_ratio.rs` + `lib.rs` mod.

## Job
TDD pure math module. Commit once: `feat(m4): commit_ratio §3.2 translation (task 1)`.
Use `$env:CARGO_TARGET_DIR = "$env:USERPROFILE\ramjob-target"` and `scripts/dev-env.ps1`.
Do not AskQuestion. Report to `.superpowers/sdd/task-40-report.md`.
