# Brief — 45 panel backstop warning

Repo: `E:/Troy/Code/Side Projects/Ram` · Branch: `milestone/m4-job-backstop`  
Ticket: `.scratch/ramjob/issues/45-panel-backstop-warning.md`

## Own
`crates/ramjob-app/ui/app.js` (+ minimal CSS/HTML if needed). SPEC already notes deferred Chromium auto-enable.

## Job
When enabling `always_enforce` / hard backstop via panel ⚙, show SPEC §7.4 copy:
"If this app can't handle running out of memory, it may crash or lose unsaved work."
`node --check` app.js. Commit: `docs(m4): SPEC backstop + panel warning (task 6)` — include any remaining SPEC/progress polish.
Report `.superpowers/sdd/task-45-report.md`.

Can run after or parallel with 44 if only touching app UI (not ramjob-core). No AskQuestion.
