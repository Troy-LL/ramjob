# Brief — 53 adaptive sleep

Repo: `E:/Troy/Code/Side Projects/Ram` · Branch: `milestone/m5-etw-budget`  
Ticket: `.scratch/ramjob/issues/53-adaptive-sleep.md`  
Design §6.1 ladder. WMI landed `a2a8828`.

## Own
- Create `crates/ramjob-core/src/adaptive.rs`
- Wire `ramjob-cli` run loop + `ramjob-app` spawn_tick_loop
- lib.rs mod

## Locked intervals
| Condition | Sleep |
|---|---|
| panel_open | 1s |
| Disarmed, panel closed | 120s |
| Armed, panel closed, max phase Idle | 15s |
| Armed, panel closed, Pressure | 3s |
| Armed, panel closed, Trim/LowYield/Thrashing (or backstop active) | 1s |

Commit: `feat(m5): adaptive polling ladder (task 4)`  
progress task 4; task-53-report.md  
Env: dev-env + USERPROFILE ramjob-target. No AskQuestion. Skip long skill reads.
