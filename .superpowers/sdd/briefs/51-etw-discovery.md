# Brief — 51 ETW discovery

Repo: `E:/Troy/Code/Side Projects/Ram` · Branch: `milestone/m5-etw-budget`  
Ticket: `.scratch/ramjob/issues/51-etw-discovery.md`  
Depends: `51ee59f` DiscoverySource exists.

## Own
`crates/ramjob-core/src/discovery/` only (+ Cargo.toml windows features if needed).

## Job
Implement `EtwProcessSource`. If full ETW consumer is too heavy for one ticket: ship a real open attempt that returns structured Err on failure + a testable event queue injector for Spawn/Exit mapping, and document live ETW as best-effort. Prefer working degrade path over fake success.

Commit: `feat(m5): ETW process discovery backend (task 2)`  
Report: `.superpowers/sdd/task-51-report.md`  
Env: dev-env.ps1 + USERPROFILE ramjob-target. No AskQuestion.
