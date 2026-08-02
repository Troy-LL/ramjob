# 30 — M3 thermo Critical/Important fixes

**Milestone:** M3  
**Depends on:** M3 merge + parity on main  
**Source:** [Thermo CQ review M3](b7af3b7f-0ef2-48ac-a9f9-6ddb18247688)

## Goal

Clear the M3 thermo gate: Critical = 0, Important = 0.

## Acceptance

1. **C1 — Always-on engine cadence.** Panel visibility changes rate/density only. Closed: full `Runtime` sweep at SPEC §6.1 rates (30 s armed / 120 s disarmed). Open: 1 s. Never stop FSM/enumerate/trim when the panel is hidden.
2. **C2 — Compose `Runtime::tick`.** Delete app-side `PathCache` + parallel enumerate. Extend `TickOutcome` (or equivalent) so the app can build panel groups from the tick’s groups without a second discovery pass.
3. **I3 — Single UI refresh owner.** Frontend must not full-DOM-rebuild every second blindly: skip `render()` when the snapshot is unchanged (fingerprint/JSON equality), and keep document-level drag commit. Do not add a second push channel in this ticket.
4. **I4 — One Win32 memory helper.** Core exports total+used (or avail+total); `gate::available_phys_bytes` and the app both use it. No `GlobalMemoryStatusEx` copy in `ramjob-app`.
5. **I5 — Pause label sync.** Tray menu Pause/Resume text updates when panel `pause_all` IPC flips pause, and the reverse path already updates the tray — one shared updater.
6. **I6 — Honest contract.** Either populate `PanelGroup.honest` from core or remove the field until §7.4. Prefer **remove** the dead IPC field; keep JS `honestMessage` fsm-based copy for now.
7. **I7 — Brainstorm junk.** If tracked under `.superpowers/brainstorm/`, remove pid/token/runtime junk from the tree (or gitignore + delete). Skip if already untracked.

Nice-to-haves (#8–#10) are **out of scope**.

## Verify

```powershell
. .\scripts\dev-env.ps1
$env:CARGO_TARGET_DIR = "$env:USERPROFILE\ramjob-target"
cargo test --workspace
cargo build -p ramjob-app
node --check crates/ramjob-app/ui/app.js
```

## Decision (autonomous)

User delegated product decisions. Close M3 after this ticket; open M4 next.
