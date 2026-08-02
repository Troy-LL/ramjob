# Brief — 46 M4 thermo fixes

Repo: `E:/Troy/Code/Side Projects/Ram` · Branch: `milestone/m4-job-backstop`  
Ticket: `.scratch/ramjob/issues/46-m4-thermo-fixes.md`  
Review: `.superpowers/sdd/m4-thermo-review.md`

## Own
`ramjob-core` only (job_backstop, runtime split, tests). Do not touch panel.

## Order
1. Fix C1 first (handle ownership) + Err-path test
2. I4 JobLimitState
3. I3 apply_backstop_limit helper
4. I5 tick dispatch cleanup
5. I2 unify mocks
6. I1 extract modules/tests

Commit once. Update progress.md task 7. Write `.superpowers/sdd/task-46-report.md`.
No AskQuestion. Env: dev-env.ps1 + USERPROFILE ramjob-target.
