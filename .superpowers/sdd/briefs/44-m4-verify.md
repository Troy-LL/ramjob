# Brief — 44 M4 hog verify

Repo: `E:/Troy/Code/Side Projects/Ram` · Branch: `milestone/m4-job-backstop`  
Ticket: `.scratch/ramjob/issues/44-m4-verify.md`  
Depends: ticket 43 committed.

## Job
1. Integration/hog proof that opt-in backstop arms (diagnostics or mock/live as feasible)
2. Prove dropping RamJob job handles does not kill hog (KILL_ON_JOB_CLOSE off)
3. Write `.superpowers/sdd/m4-verify.md` with commands + results
4. Commit: `test(m4): job backstop hog verify (task 5)`
5. Report `.superpowers/sdd/task-44-report.md`

Env: `. .\scripts\dev-env.ps1`; `$env:CARGO_TARGET_DIR="$env:USERPROFILE\ramjob-target"`
Prefer extending core tests or a focused `tests/m4_backstop.rs`; live hog OK if reliable.

No panel UI. No AskQuestion.
