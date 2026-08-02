# RamJob M4 — Job Object backstop (design)

**Date:** 2026-08-03  
**Status:** Approved by autonomous run (user delegated all product decisions 2026-08-03)  
**Milestone:** M4 (SPEC §10 — Job Object backstop with §3.2 translation, opt-in)  
**Amends:** SPEC §4.2 backstop mechanics (implementation ownership); resolves §4.2 OPEN for M4 scope  
**Depends on:** M3 closed after thermo Critical/Important = 0

---

## Problem

M2/M3 soft-trim + UI can encourage GF under a cap, but cannot stop commit growth. When soft trim fails three times in a window (or the user opts into hard backstop), RamJob must attach a Job Object with `JobMemoryLimit` derived from the GF cap via §3.2 — without killing apps on RamJob exit, and without arming a latent crash risk while the system is DISARMED.

## Goals

- One Job Object per capped group, created lazily.
- `JobMemoryLimit = 1.15 × C × clamp(commit_ratio, 1.0, 2.0)` with EMA `commit_ratio` sampled in PRESSURE; **no arm until ≥3 samples**.
- Opt-in only (`always_enforce` / hard-backstop flag already on `GroupConfig` + panel ⚙) — default off; §7.4 warning copy when enabling.
- Wire FSM `WouldBackstop` / BACKSTOP phase into real assign + limit set (replace diagnostics-only stub).
- DISARM → raise limits to unlimited (do not leave hard caps armed).
- Cap-decrease ratchet: never set limit below `current_commit × 1.05`; soft-trim then ratchet down.
- `KILL_ON_JOB_CLOSE` off; `BREAKAWAY_OK` off; nested-job failure → soft-only + honest diagnostic.
- Crash safety: drop all job handles on exit without killing members; session disconnect releases session-owned jobs (SPEC §8).
- Hog/integration proof: opt-in backstop prevents commit past limit (or fails allocations) without killing on RamJob stop.

## Non-goals (M4)

- Auto-enable Chromium-family backstop (SPEC OPEN) — **deferred**; M4 stays explicit opt-in only.
- ETW discovery / adaptive ladder CI budgets (M5).
- Autostart / preflight polish (M6).
- Redesigning soft trim or panel layout.
- Elevated helper service.

## Approaches considered

| Approach | Tradeoff | Choice |
|---|---|---|
| A. Full backstop now per SPEC §3.2 + §4.2 | Matches milestone; more Win32 surface | **Accept** |
| B. Diagnostics-only forever | Leaves product soft-only | Reject — M4 purpose |
| C. Auto-on for Chromium | Higher crash risk; OPEN unresolved | Reject for M4; revisit post-ship |

**Recommendation (locked):** A with opt-in only.

## Architecture

```
Runtime::tick
  → FSM action Backstop | (WouldBackstop becomes real)
  → BackstopEnforcer
       commit_ratio EMA (PRESSURE samples)
       translate C → JobMemoryLimit (§3.2)
       ensure Job Object per group_key
       AssignProcessToJobObject for members not yet assigned
       SetInformationJobObject(JobMemoryLimit)
  → on SystemArm::Disarmed: clear limits (unlimited) for all armed jobs
```

### New / owned units

| Unit | Responsibility |
|---|---|
| `commit_ratio` | Per-group EMA of `Σ PrivateUsage / GF`; sample count; clamp helper |
| `job_backstop` | Create/open job, set limits, assign PIDs, disarm/unlimited, Drop cleanup |
| `runtime` glue | Call backstop on FSM escalate + `always_enforce` path when armed |
| diagnostics | `BACKSTOP arm limit=…`, `BACKSTOP degrade nested`, `BACKSTOP disarm` |

Existing soft `enforcer` stays the trim path. Do not merge soft+hard into one god module.

## Config / UI

- Reuse `always_enforce` as the opt-in hard-backstop flag (already in config + `set_flags`).
- No new TOML keys required for M4 minimum. Optional later: `backstop = true` alias — **YAGNI**, skip.
- Panel ⚙ already exposes always_enforce; add honest warning string when turning on (SPEC §7.4 “Enabling backstop”).

## Verify

1. Unit: §3.2 translation + clamp + three-sample gate + ratchet math (pure).
2. Unit/integration: create job, assign hog, set low limit, observe alloc failure or commit ceiling; drop RamJob handles → hog survives.
3. `cargo test --workspace` green; CLI `ramjob run` with fixture config + hog.
4. Document known limitation: processes already in a non-nestable job stay soft-only.

## Decision log (autonomous)

| Topic | Decision |
|---|---|
| M3 UX gate | Closed after thermo C/I = 0; manual tray click-through is checklist not blocker |
| Chromium auto-backstop OPEN | Deferred past M4 |
| Flag name | Keep `always_enforce` as backstop opt-in |
| Disarm behavior | Unlimited (clear limit), keep job object handle for reuse |
