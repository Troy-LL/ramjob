# Brief — 54 runtime discovery

Repo: `E:/Troy/Code/Side Projects/Ram` · Branch: `milestone/m5-etw-budget`  
Ticket: `.scratch/ramjob/issues/54-runtime-discovery.md`  
Adaptive at `c361851`. Discovery select at `a2a8828`.

## Own
Primarily `runtime.rs` (+ scanner PathCache invalidate API if missing). Minimal CLI/app glue to construct discovery via `select_discovery`.

## Job
1. PathCache invalidate on Exit (add method if needed)
2. Each tick: poll_events; apply; optional full enumerate still OK
3. select_discovery once at Runtime::new or first tick; diagnostic once
4. Mock discovery tests
5. Commit: `feat(m5): runtime applies discovery events (task 5)`
6. progress task 5; task-54-report.md

Env: dev-env + USERPROFILE ramjob-target. No budget CI yet. No AskQuestion. Skip long skill reads.
