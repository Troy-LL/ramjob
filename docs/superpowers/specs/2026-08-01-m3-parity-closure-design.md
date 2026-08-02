# RamJob M3 parity closure (design)

**Date:** 2026-08-01  
**Status:** Approved by autonomous run (user delegated decisions)  
**Amends:** `2026-07-27-m3-tray-ui-design.md` implementation gaps found by council Loop 1  
**Trail:** `.audit/m3-parity-autonomous.tsv`

## Problem

M3 merged on `main`, but councils found the live panel could not match the approved design: Tauri IPC may never attach (`withGlobalTauri` missing → mock forever), Show-all was dead (Rust GF floor stripped groups), gauge fill ignored SPEC §7.2 scale, and drag-release could lose `pointerup` when preview re-renders destroyed the capture target.

## Goals

- Production WebView talks to Rust (`get_snapshot` / mutators), not `MOCK_SNAPSHOT`.
- Default grid: top 5 uncapped / ≥50 MB when capped; Show all expands to every group.
- Dial fill against cap when set, against total RAM when unlimited.
- Ceiling and per-app drag commit on document-level `pointerup`.
- Tray tooltip can say Warning; README status is honest about SAC build blockers.
- Fail signals (SAC, vcvars re-entry) encoded as lessons.

## Non-goals

- M4 Job Objects, friendly display-name map, full §7.4 honest-state matrix, tray screen-anchor popover geometry, panel `--simulate-armed`, CLI↔app config hot-reload.

## Approaches considered (council Loop 1–2)

| Approach | Tradeoff | Choice |
|---|---|---|
| A. Full M3 rewrite | High risk, already shipped structure | Reject |
| B. Close evidenced gaps only | Smallest path to design parity | **Accept** |
| C. Defer all UI until SAC off | Leaves mock forever for users who can build | Reject as sole path |

## Architecture notes

- `tauri.conf.json` `app.withGlobalTauri: true` exposes `window.__TAURI__` for the vanilla `ui/app.js` invoke path.
- `build_panel_groups` stops applying `meets_gf_floor`; filtering is UI-owned.
- Drag listeners bind to `document` so preview DOM rebuilds cannot drop commit.
- `commands::snapshot_from` collapses mutator boilerplate (thermo).

## Success

- Code review / unit tests green for core+cli; `node --check` on `app.js`.
- Manual or future build of `ramjob-app` with SAC allowing unsigned build scripts shows live process groups, working Show all, and dials scaled per SPEC.
- `verify-ramjob` doctor + list remain green on this machine.

## SAC blocker

Smart App Control currently blocks fresh Tauri dependency build scripts (os 4551). Turning SAC off is a one-way security change; the run does not flip it without an explicit product call. Until then, tray binary proof stays **unreachable** on this host. Source fixes are committed; they are not proven end-to-end until `cargo build -p ramjob-app` succeeds and a human drives the panel.
