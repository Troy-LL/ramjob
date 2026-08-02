# M4 Job Object Backstop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Opt-in Job Object hard backstop with §3.2 GF→commit translation, DISARM clears limits, ratchet on cap decrease, hog/integration proof.

**Architecture:** New `commit_ratio` + `job_backstop` modules in `ramjob-core`; FSM gains real `Backstop` action (replace diagnostics-only `RecordWouldBackstop`); `Runtime` arms/disarms jobs; tray/CLI reuse `always_enforce` as opt-in. No elevated service.

**Tech Stack:** Rust, `windows-rs` Job Objects (`CreateJobObjectW`, `AssignProcessToJobObject`, `SetInformationJobObject` / `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`), existing hog + config.toml.

## Global Constraints

- Product name RamJob; Windows 10 1809+ / 11 x64.
- `KILL_ON_JOB_CLOSE` must stay **off**; `BREAKAWAY_OK` off.
- Backstop **opt-in only** (`always_enforce`); Chromium auto-enable deferred (design 2026-08-03).
- Three `commit_ratio` samples required before arming; clamp ratio to `[1.0, 2.0]`; `JobMemoryLimit = 1.15 × C × clamp(...)`.
- On system DISARM: raise job limits to unlimited (do not leave latent hard caps).
- Cap decrease while armed: never set limit below `current_commit × 1.05`; soft trim then ratchet.
- Nested-job assign failure → soft-only + diagnostic; never panic.
- One commit per task; branch `milestone/m4-job-backstop`.
- Coding Tasks: `poteto-agent`, weaker Cursor model than parent, explicit `model:`.
- Do not truncate SPEC; fold OPEN resolution into §4.2 (auto-enable deferred).

**Design:** `docs/superpowers/specs/2026-08-03-m4-job-object-backstop-design.md`

---

## File map

| File | Responsibility |
|---|---|
| `crates/ramjob-core/src/commit_ratio.rs` | EMA + sample count + §3.2 translate/clamp/ratchet pure math |
| `crates/ramjob-core/src/job_backstop.rs` | Job handle map, assign, set/clear limit, Drop safety |
| `crates/ramjob-core/src/fsm.rs` | `FsmAction::Backstop`; phase if needed |
| `crates/ramjob-core/src/runtime.rs` | Wire backstop + disarm + PRESSURE sampling |
| `crates/ramjob-core/src/lib.rs` | `mod` exports |
| `crates/ramjob-app` / CLI | Honest warning when enabling always_enforce (minimal) |
| `.scratch/ramjob/issues/4x-*.md` | Tickets |
| `.superpowers/sdd/progress.md` | M4 ledger |

---

### Task 1: Pure §3.2 commit_ratio math (TDD)

**Files:**
- Create: `crates/ramjob-core/src/commit_ratio.rs`
- Modify: `crates/ramjob-core/src/lib.rs`

- [ ] **Step 1: Failing tests** for `translate_job_limit(c, ratio)`, clamp, three-sample gate (`ready()` false until 3), ratchet `max(target, commit * 1.05)`.
- [ ] **Step 2: Implement** EMA update + helpers. No Win32.
- [ ] **Step 3: `cargo test -p ramjob-core commit_ratio` green.**
- [ ] **Step 4: Commit** `feat(m4): commit_ratio §3.2 translation (task 1)`

---

### Task 2: Job Object wrapper (assign + limit + Drop)

**Files:**
- Create: `crates/ramjob-core/src/job_backstop.rs`
- Modify: `lib.rs`

- [ ] **Step 1: Failing tests** with injectable hooks trait where possible; Windows-only integration test ignored if no job APIs in CI — prefer unit tests on limit packing + “degrade on assign error” logic.
- [ ] **Step 2: Implement** `JobBackstopStore`: create job per group key, `set_memory_limit`, `clear_limit` (unlimited), `assign_pid`, Drop closes handles without kill-on-close.
- [ ] **Step 3: Verify** unit tests green; document KILL_ON_JOB_CLOSE bit never set.
- [ ] **Step 4: Commit** `feat(m4): job_backstop Job Object store (task 2)`

---

### Task 3: FSM Backstop action

**Files:**
- Modify: `crates/ramjob-core/src/fsm.rs` (+ tests)

- [ ] **Step 1: Change** `RecordWouldBackstop` → `Backstop` (or keep alias one release — prefer rename + update tests).
- [ ] **Step 2: Emit `Backstop`** after 3 ineffective trims in 60s when `always_enforce` (opt-in). If not opted in, keep soft-stop behavior / diagnostic-only path as today for non-opt-in.
- [ ] **Step 3: Tests** cover opt-in vs opt-out.
- [ ] **Step 4: Commit** `feat(m4): FSM Backstop action when opted in (task 3)`

---

### Task 4: Runtime wire-up

**Files:**
- Modify: `runtime.rs`, possibly accountant helpers for PrivateUsage sum

- [ ] **Step 1: On PRESSURE**, sample group commit (`Σ PrivateUsage`) / GF into `commit_ratio` map.
- [ ] **Step 2: On `FsmAction::Backstop`**, if ready + opted in, assign members + set limit; diagnostics line.
- [ ] **Step 3: On system DISARM**, `clear_limit` all jobs.
- [ ] **Step 4: Cap decrease path** uses ratchet helper when job already limited.
- [ ] **Step 5: Tests** with simulated groups / mock job store if factored.
- [ ] **Step 6: Commit** `feat(m4): runtime arms Job Object backstop (task 4)`

---

### Task 5: Hog integration verify

**Files:**
- Modify or create: `crates/ramjob-core/tests/m4_backstop.rs` or CLI smoke notes in `.superpowers/sdd/m4-verify.md`

- [ ] **Step 1: Spawn hog**, config with low cap + `always_enforce`, force armed / always_enforce path, assert backstop diagnostics and hog survives after runtime drop.
- [ ] **Step 2: Write** `.superpowers/sdd/m4-verify.md` with commands + results.
- [ ] **Step 3: Commit** `test(m4): job backstop hog verify (task 5)`

---

### Task 6: Panel honest warning + SPEC fold

**Files:**
- Modify: `crates/ramjob-app/ui/app.js` (warning when toggling always_enforce)
- Modify: `SPEC.md` §4.2 OPEN → deferred note; link M4 design
- Modify: `.superpowers/sdd/progress.md`

- [ ] **Step 1: UI copy** for enabling backstop (SPEC §7.4).
- [ ] **Step 2: SPEC** update untruncated.
- [ ] **Step 3: Commit** `docs(m4): SPEC backstop + panel warning (task 6)`

---

### Task 7: Milestone thermo + lessons

- [ ] **Step 1:** `cargo test --workspace` + m4-verify checklist.
- [ ] **Step 2:** Dispatch `thermo-nuclear-code-quality-review-subagent` once for M4.
- [ ] **Step 3:** Fix Critical/Important via poteto; re-verify.
- [ ] **Step 4:** Lesson capture if repeatable failure; update steering failure log.
- [ ] **Step 5: Commit** `chore(m4): thermo fixes and lessons (task 7)`

---

## Execution note

Start only after M3 thermo ticket 30 is reviewer-approved and `milestone/m3-thermo-fix` is merged or M4 branch cut from it. Controller uses poteto-mode; no two implementers on `ramjob-core` at once.
