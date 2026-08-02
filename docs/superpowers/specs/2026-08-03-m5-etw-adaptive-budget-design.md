# RamJob M5 — ETW discovery, adaptive polling, budget CI (design)

**Date:** 2026-08-03  
**Status:** Approved by autonomous run (user delegated decisions 2026-08-03)  
**Milestone:** M5 (SPEC §10)  
**Depends on:** M4 closed after thermo Critical/Important = 0  
**Amends:** SPEC §6.1 implementation ownership; §9.4 CI budget asserts

---

## Problem

M0–M4 prove soft trim + Job Object backstop + tray UI, but discovery still sweeps on a timer and there is no CI gate for §6 idle CPU / working-set ceilings. Without ETW (or a solid fallback) and adaptive cadence, the product risks burning idle budget on healthy machines.

## Goals

- Event-driven process start/stop via ETW `Microsoft-Windows-Kernel-Process` when available.
- Fallback ladder: ETW → WMI `__InstanceCreationEvent` on `Win32_Process` → sweep-only (SPEC §6.1).
- Adaptive polling ladder while ARMED (IDLE 15s / PRESSURE 3s / TRIM 1s); full sweep 30s armed / 120s disarmed when panel closed (panel open stays 1s — already M3).
- Budget instrumentation in CI against synthetic hog: fail if idle CPU > 0.3% or idle WS > 25 MB (§6 / §9.4).
- Diagnostics when discovery mode degrades (ETW unavailable → WMI → sweep).

## Non-goals (M5)

- Autostart / first-run polish (M6).
- Battery OPEN (§6.1) — **defer**: keep pressure gating identical; do not sleep engine on battery in M5 (document OPEN remains).
- Perfect ETW elevation story — non-elevated PROFILE_USER session; degrade honestly.
- Rewriting Job Object / panel.

## Approaches

| Approach | Tradeoff | Choice |
|---|---|---|
| A. Full ETW + adaptive + CI budgets | Matches SPEC M5 | **Accept** |
| B. Adaptive + CI only, skip ETW | Leaves poll-heavy discovery | Reject — M5 names ETW |
| C. ETW-only, no CI | Misses §9.4 | Reject |

## Architecture

```
DiscoverySource (trait)
  ├─ EtwProcessSource  (Kernel-Process)
  ├─ WmiProcessSource  (fallback)
  └─ SweepOnly         (last resort)

Runtime tick
  ├─ apply discovery deltas (spawn/exit) into PathCache / group membership hints
  ├─ adaptive sleep from system arm + per-group phase (panel open overrides to 1s in app)
  └─ budget sampler (optional thread / tick counters) → CI harness

CI: scripts or cargo test that runs hog-idle fixture, samples RamJob CPU/WS, asserts §6 ceilings.
```

## Decisions (locked)

| Topic | Decision |
|---|---|
| Battery OPEN | Deferred past M5; no sleep-on-battery |
| ETW failure | Degrade to WMI then sweep; diagnostic once |
| CI budgets | Hard fail on ceiling breach in CI job; document local SAC flakiness |
| Adaptive owner | Core `Runtime` / CLI/app tick sleep helpers; app panel-open 1s unchanged |

## Verify

1. Unit: discovery trait mock injects spawn/exit.
2. Integration: with ETW unavailable in CI, fallback path selected (assert diagnostic).
3. Budget test: synthetic idle measurement harness; thresholds from SPEC §6.
4. `cargo test --workspace` green.

## Success

§6 targets are measured and gated; discovery is push-based when the OS allows it.
