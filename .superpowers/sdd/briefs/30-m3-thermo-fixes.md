# Brief — 30 M3 thermo fixes

Repo: `E:/Troy/Code/Side Projects/Ram`  
Branch: `milestone/m3-thermo-fix`  
Ticket: `.scratch/ramjob/issues/30-m3-thermo-fixes.md`  
Thermo: Critical×2 + Important×5 from review `b7af3b7f-0ef2-48ac-a9f9-6ddb18247688`

## Own

- `crates/ramjob-core` (gate phys helper, `Runtime::tick` / `TickOutcome`, optional `PanelGroup.honest` removal)
- `crates/ramjob-app` (`main.rs` tick loop, `commands.rs` pause sync, `ui/app.js` render skip + honest field)

## Do not

- M4 Job Objects, ETW, nice-to-haves (split app.js, mock gate, serde enums)
- Truncate SPEC
- Ask the human — decisions are delegated; follow the ticket

## Approach (locked)

1. Add `gate::phys_memory() -> Result<(total, avail), String>` (or used); reimplement `available_phys_bytes` on top.
2. Make `Runtime::tick` return apps (or attach to `TickOutcome`) so panel can `build_panel_groups` without re-enumerate.
3. Rewrite `spawn_tick_loop` / `run_tick`: always `runtime.tick`; sleep 1s when panel visible, else 30s if Armed else 120s; history sample every open tick and on each closed full tick; tooltip every tick.
4. Pause: hold `MenuItem` (or tray id + text helper) reachable from `pause_all` command; update label whenever `set_pause_all` succeeds.
5. JS: fingerprint last snapshot; `setInterval` fetch still ok; call `render` only on change.
6. Remove `honest` from `PanelGroup` + JS mocks if present.
7. Commit once on this branch with message referencing ticket 30.

## Verify

Per ticket Verify section. Report DONE / DONE_WITH_CONCERNS with evidence.
