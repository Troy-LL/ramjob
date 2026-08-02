# M5 ETW / Adaptive Polling / Budget CI — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Event-driven process discovery (ETW→WMI→sweep), adaptive polling ladder, and CI budget asserts for SPEC §6 ceilings.

**Architecture:** `DiscoverySource` trait with ETW / WMI / sweep backends; Runtime consumes spawn/exit deltas; tick sleep derived from arm + phase (app panel-open 1s unchanged); CI harness samples idle CPU/WS against hog.

**Tech Stack:** Rust, `windows-rs` ETW / WMI as available, existing hog + cargo test / script CI.

## Global Constraints

- Windows 10 1809+ / 11; product RamJob.
- Battery OPEN deferred — no sleep-on-battery in M5.
- ETW failure degrades honestly with one diagnostic.
- Do not break M4 Job Object / M3 panel cadence contracts.
- One commit per task; branch `milestone/m5-etw-budget`.
- Coding Tasks: `poteto-agent`, weaker Cursor `model:`.
- Design: `docs/superpowers/specs/2026-08-03-m5-etw-adaptive-budget-design.md`

---

## File map

| File | Responsibility |
|---|---|
| `crates/ramjob-core/src/discovery/` | Trait + ETW / WMI / Sweep backends |
| `crates/ramjob-core/src/adaptive.rs` | Sleep interval from arm + phase (+ panel override input) |
| `crates/ramjob-core/src/budget.rs` | Sample own CPU/WS helpers for CI |
| `crates/ramjob-core/src/runtime.rs` | Consume discovery deltas |
| `crates/ramjob-app` / CLI | Use adaptive sleep when panel closed |
| `scripts/` or `tests/` | CI budget gate |
| `.superpowers/sdd/m5-verify.md` | Evidence |

---

### Task 1: DiscoverySource trait + sweep backend (TDD)

- [ ] Trait: `poll_events() -> Vec<DiscoveryEvent>` (Spawn/Exit with pid+ctime)
- [ ] `SweepDiscovery` wrapping existing enumerate diff
- [ ] Unit tests with fake process lists
- [ ] Commit: `feat(m5): DiscoverySource + sweep backend (task 1)`

### Task 2: ETW backend + degrade path

- [ ] `EtwProcessSource` for Kernel-Process when openable
- [ ] On failure return Err → caller falls back
- [ ] Diagnostic string once
- [ ] Commit: `feat(m5): ETW process discovery backend (task 2)`

### Task 3: WMI fallback

- [ ] `WmiProcessSource` between ETW and sweep
- [ ] Selector: try ETW, else WMI, else Sweep
- [ ] Commit: `feat(m5): WMI discovery fallback (task 3)`

### Task 4: Adaptive sleep helper

- [ ] Pure `next_sleep(arm, max_phase, panel_open) -> Duration` per SPEC §6.1
- [ ] Wire CLI `ramjob run` loop; app tick loop uses helper (panel open → 1s)
- [ ] Commit: `feat(m5): adaptive polling ladder (task 4)`

### Task 5: Runtime consumes discovery deltas

- [ ] Invalidate PathCache entries on Exit; hint refresh on Spawn
- [ ] Tests with mock DiscoverySource
- [ ] Commit: `feat(m5): runtime applies discovery events (task 5)`

### Task 6: Budget CI harness

- [ ] Measure RamJob idle WS (and CPU if reliable) against §6 ceilings
- [ ] `cargo test` or script that fails CI on breach; document SAC limitations
- [ ] `.superpowers/sdd/m5-verify.md`
- [ ] Commit: `test(m5): budget CI harness (task 6)`

### Task 7: Thermo + lessons

- [ ] Workspace verify + m5-verify
- [ ] One thermo CQ review; fix C/I
- [ ] Lesson capture; steering → M6
- [ ] Commit: `chore(m5): thermo fixes and lessons (task 7)`

---

## Execution

Cut `milestone/m5-etw-budget` from M4 HEAD after M4 closeout commit. Serial on `ramjob-core`.
