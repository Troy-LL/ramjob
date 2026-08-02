# Brief — 52 WMI fallback

Repo: `E:/Troy/Code/Side Projects/Ram` · Branch: `milestone/m5-etw-budget`  
Ticket: `.scratch/ramjob/issues/52-wmi-fallback.md`  
ETW landed at `e09e55e`.

## Own
`crates/ramjob-core/src/discovery/` only.

## Job
1. `WmiProcessSource` — real WMI event query if practical; else constructor that attempts COM/WMI and Errs for sweep fallback, plus inject queue for tests (same pattern as ETW).
2. `select_discovery() -> (Box<dyn DiscoverySource>, DiscoveryMode, Option<String diagnostic>)`
3. Tests: force ETW fail → WMI; both fail → Sweep.
4. Commit: `feat(m5): WMI discovery fallback (task 3)`
5. progress.md task 3; task-52-report.md

Env: dev-env.ps1 + USERPROFILE ramjob-target. No AskQuestion. No runtime wire yet (task 5).
