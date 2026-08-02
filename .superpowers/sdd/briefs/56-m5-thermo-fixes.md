# Brief — 56 M5 thermo fixes

Repo: `E:/Troy/Code/Side Projects/Ram` · Branch: `milestone/m5-etw-budget`  
Ticket: `.scratch/ramjob/issues/56-m5-thermo-fixes.md`  
Review: `.superpowers/sdd/m5-thermo-review.md`

## Own
`discovery/**`, `runtime.rs`, maybe `scanner` — ramjob-core (+ app only if needed to compile).

## Order
1. C1 + C2 (lifecycle — highest)
2. I1 Context pointer
3. I3 inert test discovery
4. I2 single enumerate
5. I4 QueuedDiscovery extract
6. I5 store DiscoveryMode
7. I6 create_time

Commit once. progress task 7. task-56-report.md.
Env: dev-env + USERPROFILE ramjob-target. SAC is Off. No AskQuestion. Skip long skill reads.
