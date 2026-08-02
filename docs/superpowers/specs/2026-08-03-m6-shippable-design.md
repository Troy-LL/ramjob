# RamJob M6 — Config, autostart, preflight, first-run (design)

**Date:** 2026-08-03  
**Status:** Approved by autonomous run (user delegated decisions; SAC Off unblocked M5 verify)  
**Milestone:** M6 (SPEC §10 — Shippable)  
**Depends on:** M5 thermo Critical/Important = 0  
**Amends:** SPEC §5.4, §7.3, §8.3 implementation ownership

---

## Problem

M0–M5 deliver enumerate → soft trim → policy → tray UI → Job Object backstop → ETW/adaptive/budget. Shipping still needs: reliable config lifecycle polish, HKCU Run autostart, startup preflight (§5.4), and first-run UX without a wizard (§7.3).

## Goals

- Config: version=2 lifecycle already exists — harden 90-day prune, `pinned`, atomic save (audit gaps only).
- Autostart: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value for RamJob; tray/Settings toggle.
- Preflight once at startup: pagefile, RAM ≥32 GB dormancy note, privilege notes → diagnostics + first-run copy.
- First-run: no wizard; panel shows top consumers + one-line explainer; high-RAM dormancy note from preflight.
- Quit / Pause / Open already M3 — ensure Settings stub enables autostart at minimum.

## Non-goals (M6)

- Elevated helper service.
- Store / signed installer polish beyond “runs from cargo build / local install path”.
- Battery OPEN (still deferred).
- Chromium auto-backstop OPEN (still deferred).
- Full §7.4 honest-state matrix beyond what M3/M4 already show.

## Approaches

| Approach | Tradeoff | Choice |
|---|---|---|
| A. Minimal ship: autostart + preflight + first-run copy | Matches SPEC M6 | **Accept** |
| B. Full installer + code signing | Out of scope for solo milestone | Reject |
| C. Service-based autostart | SPEC says HKCU Run | Reject |

## Architecture

```
ramjob-app startup
  → preflight::run_once() → PreflightReport (cached in AppState)
  → load config / prune stale
  → tray + optional Run key sync from config.autostart
  → panel snapshot includes first_run / preflight flags for UI hint
```

Config addition: `autostart = true|false` (default true on first create, or false until user opts in — **locked: default false**, enable from Settings/tray to avoid surprise).

## Decisions (locked)

| Topic | Decision |
|---|---|
| Autostart default | **Off** until user enables |
| Preflight | Once per process start; results in diagnostics + panel first-run |
| Settings | Enable: Autostart toggle (was disabled in M3) |
| Installer | Document `cargo build -p ramjob-app --release` + copy; no MSI in M6 |

## Verify

1. Toggle autostart → Run key appears/disappears.
2. Preflight diagnostics non-empty on start.
3. First-run hint when no caps set (M3 already has related UI — wire preflight note).
4. `cargo test` + manual tray smoke.

## Success

User can install-run, enable autostart, see honest preflight, set a cap, and leave RamJob resident — shippable v0.
