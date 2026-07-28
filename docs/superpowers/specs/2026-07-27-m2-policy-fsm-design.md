# RamJob M2 — Policy FSM + Pressure Gating (design)

**Date:** 2026-07-27  
**Status:** Approved in brainstorming; ready for implementation plan  
**Milestone:** M2 (SPEC §10)  
**Supersedes / amends:** SPEC §4.1 OPEN (runaway force-arm), §4.2 runtime yield cutoff usage

---

## Problem

M0/M1 proved grouping, GF, soft trim, and compression yield. Without pressure gating and a per-group policy FSM, RamJob would trim whenever a cap exists — including on a machine with plenty of RAM — and could thrash. M2 must prove enforcement is **correct and does not thrash**.

## Goals

- System **Armed / Disarmed** from OS memory-resource notifications + hard-fault confirm (SPEC §4.1).
- Per-group FSM: IDLE → PRESSURE → TRIM, with LOW_YIELD / THRASHING stops (SPEC §4.2). Soft trim only.
- Caps and policy knobs from **config.toml** (no tray UI; that is M3).
- Apply M1 lessons: `Ry_live` for runtime yield, trim lock covers settle, no dual ΔGF pipelines, no panic on NtQSI/trim errors.
- Process: branch per milestone, **one commit per SDD task**, steering `.mdc` updated on approach failures.

## Non-goals (M2)

- Tray / panel / sliders (M3).
- Job Object hard backstop (M4). FSM may record `WouldBackstop` only.
- ETW / budget CI (M5).
- Autostart / first-run polish (M6).
- Calibrating `Ry_live` cutoff from a full regression (keep placeholder **0.35** unless evidence says raise; document in verify notes).

## Decisions (brainstorming)

| Topic | Decision |
|---|---|
| Scope | Full SPEC M2: pressure + FSM; BACKSTOP deferred to M4 |
| Caps | `config.toml` only |
| Runaway | Force-arm when `GF ≥ runaway_multiplier × C` while DISARMED; default multiplier **3.0**, **configurable** |
| Architecture | Pure `policy` + `fsm` in `ramjob-core`; thin `ramjob run` loop in CLI |
| Verify | Synthetic/simulated pressure = merge gate; optional live LowMemory smoke |
| Git | One commit per task/phase; steering mdc + lessons on failures |

## Architecture

```
config.toml ──► ramjob run (loop)
                    │
                    ├─► pressure signals (OS or inject)
                    ├─► scanner → grouper → accountant (GF)
                    ├─► policy: Disarmed | Armed
                    ├─► fsm per capped group → Decision
                    ├─► enforcer soft_trim (if Trim) + yield_math Ry_live
                    └─► diagnostics ring
```

### Components

| Unit | Responsibility |
|---|---|
| `config` | Parse versioned TOML; group key → cap / always_enforce; global runaway_multiplier |
| `policy` | Pure Armed/Disarmed transitions from notification + fault-rate samples + dwell |
| `fsm` | Pure per-group state + next action from GF, C, history, system arm, runaway, always_enforce |
| `diagnostics` | Fixed-size ring of decision records |
| `ramjob run` | Wait (notify/timer), glue pipeline, Ctrl+C cleanup |

Existing: `scanner`, `grouper`, `accountant`, `enforcer`, `yield_math`, `gate` (bench CLI remains).

## Config shape

```toml
version = 2
runaway_multiplier = 3.0

[[group]]
key = "c:\\users\\troyl\\appdata\\local\\bravesoftware"
cap_bytes = 4294967296
always_enforce = false
```

- Missing/unknown `version` → backup to `config.bak`, write fresh empty config (SPEC §8.3).
- Unknown group keys ignored until the app appears.
- `cap_bytes = 0` or omitted → unlimited (no FSM enforce).

## System pressure

```
ARM    = LowMemoryResourceNotification signaled && hard_faults/s > 30, sustained 20 s
DISARM = HighMemoryResourceNotification signaled, sustained 60 s
```

Injectable `PressureSource` + clock for tests. Simulated Armed is valid for the merge gate.

## Per-group FSM

Evaluated when system **Armed**, or `always_enforce`, or runaway (`GF ≥ multiplier × C`):

| Condition | State / action |
|---|---|
| GF < 0.85C | Idle — no trim |
| 0.85C ≤ GF < C | Pressure — fast poll intent; sample commit_ratio stub for M4 |
| GF ≥ C | Trim — soft trim toward GF < 0.9C; 20 s/group rate limit; global trim lock |
| 3 ineffective trims in 60 s | Record `WouldBackstop` (no Job Object) |
| Ry_live < cutoff twice | LowYield — stop trimming group |
| Refault >90% pre-trim in 5 s, twice | Thrashing — stop |

Foreground / visible-window exclusion remains in enforcer (M1). Measurement protocol: lock across settle (lesson).

## Loop

1. Load config (fail loud on parse error).
2. Create notification handles (or fakes).
3. Wait: notification or timer (Disarmed slow; Armed / Pressure / Trim faster per SPEC ladder).
4. Pipeline tick as in architecture diagram.
5. Quit: drop lock, close handles, exit 0 residue.

## Errors

- Config parse → exit non-zero with message; do not half-run.
- Enumerate/trim errors → diagnostics + skip; never `.expect` on NtQSI.
- Live ARM unavailable → document; do not fail milestone if synthetic suite passes.

## Verification

**Merge gate (required)**
- Unit: policy dwell, FSM matrix, runaway, always_enforce, LowYield, Thrashing, rate limit.
- Integration: hog + cap + simulated Armed → Trim without violating 20 s rate; LowYield path exercisable with hooks.

**Optional**
- One documented live LowMemory ARM smoke on a real machine.

**Milestone end**
- Thermo-nuclear CQ once; lesson capture; update steering `.mdc` if anything failed.

## Process / steering

- Branch: `milestone/m2-policy-fsm`.
- Commits: one per SDD ticket (or named phase).
- Always-on rule: `.cursor/rules/ramjob-milestone-steering.mdc`.
- Failures → update that rule and/or `.cursor/skills/ramjob-milestone-sdd/lessons/`.

## SPEC amendments (to apply with this design)

1. Resolve §4.1 OPEN: runaway force-arm **yes**, default **3×**, key `runaway_multiplier` configurable in config.
2. M2 deliverable note: soft-trim FSM + pressure; Job Object still M4; `WouldBackstop` telemetry only.
3. Runtime LOW_YIELD uses **Ry_live** with cutoff placeholder 0.35 until calibrated.

## Open follow-ups (not blocking M2)

- Fit Ry_live cutoff from paired M1 bench samples.
- Multi-machine gate corpus (Chrome/Teams/Steam).
- `always_enforce` UX copy (M3 panel).
