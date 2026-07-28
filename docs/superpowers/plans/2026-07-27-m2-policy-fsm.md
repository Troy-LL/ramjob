# M2 Policy FSM + Pressure Gating Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `ramjob run`: config-driven caps, system Armed/Disarmed pressure gating, per-group soft-trim FSM with LOW_YIELD/THRASHING stops, without Job Object backstop.

**Architecture:** Pure `config` / `policy` / `fsm` / `diagnostics` in `ramjob-core`; thin CLI loop loads config, waits on injectable pressure + timer, runs scan→group→GF→policy→fsm→enforcer→Ry_live. OS memory-resource notifications sit behind a `PressureSource` trait so tests simulate Armed.

**Tech Stack:** Rust 2021, `windows` crate (existing), `toml` + `serde` for config, existing `enforcer` / `yield_math` / `scanner` / `grouper` / `accountant`.

## Global Constraints

- Branch: `milestone/m2-policy-fsm` (already exists).
- One git commit per task below; message references task id/title.
- Coding Tasks: `subagent_type: "poteto-agent"`, explicit `model:` weaker than parent, same family (Cursor→Cursor).
- Prefer SDD; follow `.cursor/rules/ramjob-milestone-steering.mdc`.
- `Ry_live` cutoff placeholder: `0.35` (`RY_LIVE_CUTOFF`).
- `runaway_multiplier` default: `3.0`.
- Trim: reuse `with_trim_lock` / lock-across-settle for measured trims; no `.expect` on NtQSI.
- No Tauri, no Job Objects, no tray (M3/M4).
- `. .\scripts\dev-env.ps1` before cargo when link fails.
- Design SOT: `docs/superpowers/specs/2026-07-27-m2-policy-fsm-design.md`. Product SOT: `SPEC.md` (do not truncate).

## File map

| File | Responsibility |
|---|---|
| `crates/ramjob-core/src/config.rs` | Parse versioned TOML; `RamjobConfig` |
| `crates/ramjob-core/src/diagnostics.rs` | 1024-entry ring of decision strings/records |
| `crates/ramjob-core/src/policy.rs` | Pure Armed/Disarmed + dwell |
| `crates/ramjob-core/src/fsm.rs` | Pure per-group FSM → `FsmAction` |
| `crates/ramjob-core/src/pressure.rs` | `PressureSource` trait + `SimulatedPressure` + Win32 adapter |
| `crates/ramjob-core/src/runtime.rs` | One tick of the daemon pipeline (testable) |
| `crates/ramjob-cli/src/run.rs` | `ramjob run` CLI + wait loop |
| `crates/ramjob-cli/src/main.rs` | Wire `run` command |
| `crates/ramjob-core/Cargo.toml` | Add `serde`, `toml` |
| `.scratch/ramjob/issues/2x-*.md` | M2 tickets mirroring tasks |
| `.superpowers/sdd/m2-verify.md` | Verify notes |

---

### Task 1: Config TOML (`config`)

**Files:**
- Create: `crates/ramjob-core/src/config.rs`
- Modify: `crates/ramjob-core/Cargo.toml` (add `serde = { version = "1", features = ["derive"] }`, `toml = "0.8"`)
- Modify: `crates/ramjob-core/src/lib.rs` (`pub mod config;`)
- Test: unit tests inside `config.rs`

**Interfaces:**
- Consumes: nothing from prior M2 tasks
- Produces:
```rust
pub struct RamjobConfig {
    pub version: u32,
    pub runaway_multiplier: f64,
    pub groups: Vec<GroupConfig>,
}
pub struct GroupConfig {
    pub key: String,
    pub cap_bytes: u64,       // 0 = unlimited
    pub always_enforce: bool,
}
pub fn parse_config(toml_str: &str) -> Result<RamjobConfig, String>;
pub fn load_config_file(path: &Path) -> Result<RamjobConfig, String>;
pub const DEFAULT_RUNAWAY_MULTIPLIER: f64 = 3.0;
pub const CONFIG_VERSION: u32 = 2;
```

- [ ] **Step 1: Add deps + failing test**

In `config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_version_groups_and_defaults() {
        let c = parse_config(r#"
version = 2
runaway_multiplier = 3.0
[[group]]
key = "c:\\users\\x\\bravesoftware"
cap_bytes = 4294967296
always_enforce = false
"#).unwrap();
        assert_eq!(c.version, 2);
        assert_eq!(c.runaway_multiplier, 3.0);
        assert_eq!(c.groups.len(), 1);
        assert_eq!(c.groups[0].cap_bytes, 4294967296);
    }
    #[test]
    fn rejects_unknown_version() {
        assert!(parse_config("version = 99\n").is_err());
    }
    #[test]
    fn missing_multiplier_defaults_to_3() {
        let c = parse_config("version = 2\n").unwrap();
        assert_eq!(c.runaway_multiplier, DEFAULT_RUNAWAY_MULTIPLIER);
    }
}
```

- [ ] **Step 2: Run test — expect FAIL**

```powershell
. .\scripts\dev-env.ps1
cargo test -p ramjob-core config::
```

Expected: compile fail or test fail (module missing).

- [ ] **Step 3: Implement `parse_config` / `load_config_file`**

Reject `version != 2`. Default `runaway_multiplier` to `3.0`. Default `always_enforce` to false. `cap_bytes` default 0.

- [ ] **Step 4: Run tests — expect PASS**

```powershell
cargo test -p ramjob-core config::
```

- [ ] **Step 5: Commit**

```bash
git add crates/ramjob-core/Cargo.toml crates/ramjob-core/src/lib.rs crates/ramjob-core/src/config.rs Cargo.lock
git commit -m "feat(m2): parse RamJob config.toml (task 1)"
```

---

### Task 2: Diagnostics ring

**Files:**
- Create: `crates/ramjob-core/src/diagnostics.rs`
- Modify: `crates/ramjob-core/src/lib.rs`

**Interfaces:**
- Produces:
```rust
pub struct DiagnosticsRing { /* capacity 1024 */ }
impl DiagnosticsRing {
    pub fn new() -> Self;
    pub fn push(&mut self, line: impl Into<String>);
    pub fn lines(&self) -> Vec<&str>; // oldest→newest, at most 1024
}
```

- [ ] **Step 1: Failing test** — push 1025 lines; `lines().len() == 1024`; first surviving is the second pushed.

- [ ] **Step 2: Run — FAIL**

```powershell
cargo test -p ramjob-core diagnostics::
```

- [ ] **Step 3: Implement ring buffer** (`VecDeque`).

- [ ] **Step 4: PASS**

- [ ] **Step 5: Commit** `feat(m2): diagnostics ring (task 2)`

---

### Task 3: System policy (Armed/Disarmed)

**Files:**
- Create: `crates/ramjob-core/src/policy.rs`
- Modify: `crates/ramjob-core/src/lib.rs`

**Interfaces:**
- Produces:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemArm { Disarmed, Armed }

pub struct PressureSample {
    pub low_memory: bool,
    pub high_memory: bool,
    pub hard_faults_per_sec: f64,
    pub now: Instant,
}

pub struct PolicyState {
    pub arm: SystemArm,
    // internal dwell tracking
}

pub const ARM_DWELL: Duration = Duration::from_secs(20);
pub const DISARM_DWELL: Duration = Duration::from_secs(60);
pub const HARD_FAULT_ARM_THRESHOLD: f64 = 30.0;

impl PolicyState {
    pub fn new() -> Self; // starts Disarmed
    pub fn update(&mut self, sample: PressureSample) -> SystemArm;
}
```

Rules (verbatim from design):
- ARM candidate: `low_memory && hard_faults_per_sec > 30` sustained `ARM_DWELL`.
- DISARM candidate: `high_memory` sustained `DISARM_DWELL`.
- Leaving Armed requires DISARM dwell; leaving Disarmed requires ARM dwell.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn stays_disarmed_without_dwell() { /* low+faults for 10s → still Disarmed */ }
#[test]
fn arms_after_20s_low_and_faults() { /* … */ }
#[test]
fn disarms_after_60s_high() { /* from Armed */ }
#[test]
fn twitchy_low_without_faults_does_not_arm() { /* low_memory true, faults 0 */ }
```

- [ ] **Step 2: FAIL** `cargo test -p ramjob-core policy::`

- [ ] **Step 3: Implement pure dwell FSM**

- [ ] **Step 4: PASS**

- [ ] **Step 5: Commit** `feat(m2): system Armed/Disarmed policy (task 3)`

---

### Task 4: Per-group FSM

**Files:**
- Create: `crates/ramjob-core/src/fsm.rs`
- Modify: `crates/ramjob-core/src/lib.rs`

**Interfaces:**
- Consumes: `SystemArm` from policy; config caps conceptually (`cap_bytes`, `always_enforce`, `runaway_multiplier`)
- Produces:
```rust
pub const RY_LIVE_CUTOFF: f64 = 0.35;
pub const IDLE_RATIO: f64 = 0.85;
pub const TRIM_TARGET_RATIO: f64 = 0.9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupPhase {
    Idle,
    Pressure,
    Trim,
    LowYield,
    Thrashing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsmAction {
    None,
    SoftTrim,
    RecordWouldBackstop,
}

pub struct GroupFsm {
    pub phase: GroupPhase,
    // consecutive low_yield / thrash / ineffective trim counters + timestamps
}

pub struct GroupFsmInput {
    pub gf: u64,
    pub cap_bytes: u64, // 0 = unlimited → force Idle/None
    pub system: SystemArm,
    pub always_enforce: bool,
    pub runaway_multiplier: f64,
    pub now: Instant,
    pub last_ry_live: Option<f64>,      // after a trim
    pub refault_hot: bool,              // caller detected >90% return in 5s
    pub trim_was_ineffective: bool,     // caller: did not get below 0.9C
}

impl GroupFsm {
    pub fn new() -> Self;
    pub fn step(&mut self, input: GroupFsmInput) -> FsmAction;
    pub fn is_active(&self, input: &GroupFsmInput) -> bool; // armed|always|runaway
}
```

Active when `system == Armed || always_enforce || (cap > 0 && gf as f64 >= runaway_multiplier * cap as f64)`.

While active and not LowYield/Thrashing:
- `gf < 0.85*cap` → Idle, None
- `0.85*cap <= gf < cap` → Pressure, None
- `gf >= cap` → Trim, SoftTrim (caller rate-limits 20s)
- 3 ineffective in 60s → RecordWouldBackstop (still SoftTrim or None once; record once per window)
- two consecutive `last_ry_live < RY_LIVE_CUTOFF` → LowYield, None thereafter until reset API if any
- two consecutive `refault_hot` → Thrashing

Unlimited `cap_bytes == 0` → always Idle/None.

- [ ] **Step 1: Failing matrix tests** (idle/pressure/trim/runaway while Disarmed/always_enforce/low_yield/thrash/unlimited)

- [ ] **Step 2: FAIL** `cargo test -p ramjob-core fsm::`

- [ ] **Step 3: Implement**

- [ ] **Step 4: PASS**

- [ ] **Step 5: Commit** `feat(m2): per-group policy FSM (task 4)`

---

### Task 5: PressureSource + simulated pressure

**Files:**
- Create: `crates/ramjob-core/src/pressure.rs`
- Modify: `crates/ramjob-core/src/lib.rs`

**Interfaces:**
```rust
pub trait PressureSource {
    fn sample(&mut self) -> Result<PressureSample, String>;
}

pub struct SimulatedPressure {
    pub low_memory: bool,
    pub high_memory: bool,
    pub hard_faults_per_sec: f64,
}
impl PressureSource for SimulatedPressure { … }

/// Win32 CreateMemoryResourceNotification + hard-fault counter.
/// May return Err on unsupported hosts; run loop logs and stays Disarmed.
pub struct WinPressure;
impl PressureSource for WinPressure { … }
```

Hard-faults/s: read `SYSTEM_PERFORMANCE_INFORMATION` or equivalent via windows-rs if available; if too heavy for M2, approximate with `0.0` in `WinPressure` but still honor low/high notification handles, and document that simulated path covers fault confirm in tests. Prefer implementing a real counter if a single NtQuery call already exists in scanner patterns.

- [ ] **Step 1: Test SimulatedPressure feeds PolicyState to Armed**

- [ ] **Step 2: FAIL**

- [ ] **Step 3: Implement trait + Simulated + Win stub/adapter**

- [ ] **Step 4: PASS** (unit). Manual: compile `WinPressure` construction.

- [ ] **Step 5: Commit** `feat(m2): PressureSource + simulation (task 5)`

---

### Task 6: Runtime tick (glue)

**Files:**
- Create: `crates/ramjob-core/src/runtime.rs`
- Modify: `crates/ramjob-core/src/lib.rs`

**Interfaces:**
```rust
pub struct Runtime {
    pub config: RamjobConfig,
    pub policy: PolicyState,
    pub groups: HashMap<String, GroupFsm>, // key = group_key
    pub rates: HashMap<String, Instant>,   // last trim per group
    pub diagnostics: DiagnosticsRing,
}

pub struct TickOutcome {
    pub system: SystemArm,
    pub trims_attempted: usize,
}

impl Runtime {
    pub fn from_config(config: RamjobConfig) -> Self;
    /// One pipeline pass. `pressure` already sampled by caller into Policy via sample OR pass source.
    pub fn tick<P: PressureSource>(
        &mut self,
        pressure: &mut P,
        now: Instant,
    ) -> Result<TickOutcome, String>;
}
```

`tick` steps:
1. `pressure.sample()` → `policy.update`
2. `enumerate_processes_with_cache` + `group_processes` + `group_footprint`
3. For each config group whose `key` matches an `AppGroup.group_key`:
   - build `GroupFsmInput` (runaway/always/system)
   - `fsm.step` → if SoftTrim and rate ok, measured trim under lock with 3s settle using existing yield helpers; update LowYield via `last_ry_live`
   - push diagnostics lines
4. Return outcome

Reuse gate/enforcer measurement patterns; do **not** invent a second ΔGF path. Prefer extracting a shared `measure_trim_ry_live(group) -> Option<f64>` in `runtime` or `enforcer` if duplication appears.

- [ ] **Step 1: Unit test with SimulatedPressure + synthetic AppGroup injection**  
  If live enumerate is awkward, add `tick_with_groups(&mut self, sample, groups, now)` for tests and have `tick` call enumerate then that.

- [ ] **Step 2: FAIL**

- [ ] **Step 3: Implement**

- [ ] **Step 4: PASS** `cargo test -p ramjob-core runtime::`

- [ ] **Step 5: Commit** `feat(m2): runtime tick pipeline (task 6)`

---

### Task 7: `ramjob run` CLI

**Files:**
- Create: `crates/ramjob-cli/src/run.rs`
- Modify: `crates/ramjob-cli/src/main.rs`

**CLI:**
```
ramjob run [--config <path>] [--once] [--simulate-armed]
```
- Default config path: `%APPDATA%\RamJob\config.toml` (create parent dir; if missing, write empty `version = 2` template and exit 0 with message, or keep running with empty groups — **prefer write template + continue with zero caps**).
- `--once`: single tick then exit.
- `--simulate-armed`: use `SimulatedPressure { low: true, faults: 40, high: false }` and advance policy with faked Instants in a short loop OR force `policy.arm = Armed` for demo (document). Prefer driving dwell by looping with `std::thread::sleep` only when not `--once`; for `--once --simulate-armed`, set policy Armed directly for the tick (test hook) via `Runtime` method `force_arm_for_test`.

Loop: sleep 1s (Armed) or 15s (Disarmed) between ticks; Ctrl+C exits clean.

- [ ] **Step 1: Test parse_run_args**

- [ ] **Step 2: FAIL**

- [ ] **Step 3: Implement run + wire main + help text**

- [ ] **Step 4:**  
```powershell
cargo test -p ramjob-cli
cargo run -p ramjob-cli -- run --help
cargo run -p ramjob-cli -- run --once --simulate-armed --config path\to\temp.toml
```
Expected: exit 0.

- [ ] **Step 5: Commit** `feat(m2): ramjob run daemon loop (task 7)`

---

### Task 8: Integration verify (hog + simulated arm)

**Files:**
- Create: `.superpowers/sdd/m2-verify.md`
- Create/update: `.scratch/ramjob/issues/20-m2-integration.md` (optional ledger)
- Test: `crates/ramjob-core/src/runtime.rs` integration test or `crates/ramjob-cli` ignored live test

**Steps:**
- [ ] **Step 1: Automated test** — config with cap below hog GF; SimulatedPressure armed; `tick_with_groups` or full tick against live hog if spawned; assert `trims_attempted >= 1` and second tick within 20s does not trim again (rate limit).

- [ ] **Step 2: Manual (optional)** — start hog, `ramjob run --simulate-armed --config …`, observe diagnostics.

- [ ] **Step 3: Write `m2-verify.md`** with commands + Pass/Fail of synthetic gate; note live LowMemory smoke skipped/done.

- [ ] **Step 4:** `cargo test --workspace`

- [ ] **Step 5: Commit** `test(m2): integration verify + m2-verify.md (task 8)`

---

### Task 9: Milestone thermo + lessons

**Files:** steering `.mdc` failure log / lessons as needed

- [ ] **Step 1:** Dispatch `thermo-nuclear-code-quality-review-subagent` on M2 diff; write `.superpowers/sdd/m2-thermo-review.md`

- [ ] **Step 2:** Fix Critical/Important via poteto-agent (weaker in-family model)

- [ ] **Step 3:** Re-run `cargo test --workspace`

- [ ] **Step 4:** Lesson capture; update steering failure log if needed

- [ ] **Step 5: Commit** `chore(m2): thermo fixes and lessons (task 9)`

---

## Self-review (plan vs design)

| Design requirement | Task |
|---|---|
| config.toml caps + runaway_multiplier | 1 |
| diagnostics ring | 2 |
| ARM/DISARM dwell + faults | 3, 5 |
| Per-group FSM + LowYield/Thrash/WouldBackstop | 4 |
| Simulated + Win pressure | 5 |
| run loop + tick | 6, 7 |
| Synthetic verify | 8 |
| Thermo / lessons | 9 |
| No Job Object | respected (WouldBackstop only) |
| Ry_live runtime | 6 |
| Commit per task | each task step 5 |

Placeholder scan: none intentional. Types aligned across tasks (`SystemArm`, `PressureSample`, `RamjobConfig`, `FsmAction`).
