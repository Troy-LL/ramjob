# M2 Thermo-Nuclear Code Quality Review

**Scope:** M2 (`427507b`..`12da86d` on `milestone/m2-policy-fsm`) — `config`, `diagnostics`, `policy`, `fsm`, `pressure`, `runtime`, `lib.rs` exports, `Cargo.toml`, `ramjob-cli` `run`/`main`, `m2_integration`  
**Ignored:** M0/M1 except enforcer soft-trim coupling; M3 tray; M4 Job Objects  
**Rubric:** thermo-nuclear-code-quality-review (structure, 1k-line, spaghetti, code judo, abstraction quality)  
**Design truth:** `docs/superpowers/specs/2026-07-27-m2-policy-fsm-design.md`  
**Verdict:** **do not approve** — pure `policy`/`fsm` layers are clean, but `runtime` wires FSM inputs with stubs and wrong contracts (`refault_hot`, ineffective-trim semantics, `ExclusionPolicy::None`), live `WinPressure` cannot satisfy the ARM predicate, and trim measurement re-implements `gate::measure_under_lock` instead of reusing it. M2 verify asks for LowYield/Thrashing paths; only unit tests exercise those today.

**Line counts (approx):**  
`config.rs` 95 · `diagnostics.rs` 38 · `policy.rs` 123 · `fsm.rs` 242 · `pressure.rs` 97 · `runtime.rs` 242 · `run.rs` 147 · `m2_integration.rs` 132 — **no 1k breach**.

---

## Findings (priority order)

### 1. Structural / protocol regression

`crates/ramjob-core/src/runtime.rs:154` + `crates/ramjob-core/src/enforcer.rs:37-39` **blocker**: daemon soft-trim uses `ExclusionPolicy::None`. Design and M1 require foreground/visible-window exclusion in production enforcement; `None` is the bench/gate path. Runtime is the live consumer — it should default to `ProtectInteractive`. Trimming the focused app is the wrong ownership model for a policy daemon.

`crates/ramjob-core/src/runtime.rs:92-94` + `crates/ramjob-core/src/fsm.rs:100-107` **blocker**: `refault_hot` is hard-coded `false` in every `GroupFsmInput`. Thrashing is implemented and unit-tested in `fsm`, but the orchestration layer never supplies the signal. M2 verify explicitly calls for a Thrashing path; as shipped it is dead code behind a pure-FSM façade.

`crates/ramjob-core/src/runtime.rs:113-118` + `docs/superpowers/plans/2026-07-27-m2-policy-fsm.md:272` **blocker**: `trim_was_ineffective` is set from `ry_live < 0.1`, but the plan/SPEC contract is “did not get below 0.9C” (GF target). Low-yield and ineffective-trim are different stop rules conflated on one field. `WouldBackstop` counting and diagnostics lie about trim effectiveness. The adjacent `estimate_group_gf_after(app, &[])` / `ineffective` block computes nothing useful (`&[]` → pre-trim GF) and is discarded with `let _ = ineffective`.

`crates/ramjob-core/src/pressure.rs:72-75` + `crates/ramjob-core/src/policy.rs:40-41` **blocker**: `WinPressure` always reports `hard_faults_per_sec = 0.0` unless a dev-only `assume_faults_when_low` flag is set (never wired in `run.rs`). SPEC §4.1 ARM requires `low_memory && faults > 30` sustained 20s. Live `ramjob run` therefore **cannot** transition Disarmed→Armed from OS pressure alone — only `always_enforce` or runaway GF. Comment in `pressure.rs` papers this over as intentional; that is a structural split between policy purity and a non-functional live adapter.

### 2. Code judo / dramatic simplification

`crates/ramjob-core/src/runtime.rs:136-172` + `crates/ramjob-core/src/gate.rs:227-278` **important**: `measured_soft_trim` re-implements the M1 §2.3 protocol (pre-sample, `soft_trim_group_unlocked`, 3s settle, post-sample, `measure_ry_live`) that already lives in `gate::measure_under_lock` / `run_gate_on_group`. Two measurement owners will drift (gate has `require_real_trim`, intersect refresh, `available_phys`; runtime skips them). **Fix (judo):** extract a shared `measure_soft_trim_yield(group, ctx, settle) -> Result<Option<f64>, _>` used by gate and runtime, or call a slim runtime-facing wrapper around `measure_under_lock` and read `ry_live` from `GateMeasurement`.

`crates/ramjob-core/src/runtime.rs:113-116` **important**: dead stub path — `target`, `gf_after`, `ineffective` computed then thrown away; real feedback uses the wrong signal (see blocker above). Delete the stub or wire post-trim GF from the measurement closure’s post-sample maps.

`crates/ramjob-core/src/config.rs:49-53` + `SPEC.md` §8.3 **important**: unknown `version` returns `Err` only. SPEC/design require backup to `config.bak` and regenerate fresh empty config. `ensure_config` in `run.rs` then exits — user data preserved but contract unmet; worse on existing bad version (no backup, no recovery path).

### 3. Spaghetti / branching / silent modes

`crates/ramjob-core/src/runtime.rs:156` + `crates/ramjob-core/src/runtime.rs:110-112` **important**: `soft_trim_group_unlocked` outcome is ignored (`let _outcome = …`); `trims_attempted` increments and rate map updates even when inner trim rate-limited or trimmed nobody. M1 gate fail-closed lesson (`require_real_trim`) does not apply here — false-positive trim accounting and diagnostics.

`crates/ramjob-core/src/runtime.rs:141` + `runtime.rs:162` **important**: `compress_store_ws(&procs).unwrap_or(0)` silently treats store-WS read failure as zero compression delta, skewing `Ry_live` toward “good yield” instead of surfacing `Result` through the tick (design: enumerate/trim errors → diagnostics + skip, never panic — not swallow).

`crates/ramjob-cli/src/run.rs:96-118` **important**: when `WinPressure::new()` fails, loop falls back to a `SimulatedPressure` forced Disarmed-leaning (`high_memory = true`). Reasonable degrade, but the branch duplicates pressure-source selection and hides “live ARM dead” behind a warning + silent sim — orchestration belongs in core or a single `PressureSource` enum, with explicit diagnostic when fault confirm is unavailable.

`crates/ramjob-core/src/runtime.rs:97-125` **important**: double `fsm.step` in the `SoftTrim` arm (decide → trim → follow) with `GroupFsmInput` rebuilt from stale pre-trim `gf`. Works for rate-limit/LowYield only because FSM state persists across ticks; readers must hold two step stories per trim. **Fix:** split `decide(input)` / `observe_post_trim(input, measure)` or return a `TrimFeedback` struct from one orchestration helper so post-trim fields are mandatory, not optional tail fields on a copy struct.

### 4. Boundary / abstraction / type contract

`crates/ramjob-core/src/fsm.rs:38-48` **suggestion**: `GroupFsmInput` bundles pre-trim GF, post-trim `last_ry_live`, `refault_hot`, and `trim_was_ineffective` without type-level phase separation — easy to pass post-trim flags on pre-trim steps (runtime always zeroes them on line 92-94). A `PostTrimObservation { ry_live, gf_after, refault_hot }` optional on a second method would make the contract enforceable.

`crates/ramjob-core/src/diagnostics.rs:19-27` **suggestion**: string lines are fine for M2, but `runtime` formats ad-hoc `format!(…)` strings; when M3 surfaces LOW_YIELD/THRASHING, promote typed `DiagnosticEvent` now to avoid a second string protocol.

`crates/ramjob-core/src/runtime.rs:77` **suggestion**: `self.config.groups.clone()` every tick — borrow `&self.config.groups` in the loop instead.

### 5. File-size / decomposition

No file crosses 1k. `runtime.rs` (~240 product lines) is the right size **if** measurement moves to shared gate/enforcer helper and FSM feedback stops being inline stubs. Do not split `fsm`/`policy` — they are appropriately pure.

### 6. Modularity (what is working)

- `policy.rs` — small, pure dwell machine; tests cover arm/disarm/twitchy-fault edge.
- `fsm.rs` — pure per-group state; matrix tests for idle/pressure/trim/runaway/always_enforce/LowYield/thrash/WouldBackstop.
- `config.rs` — thin serde boundary; version gate is explicit (backup aside).
- `diagnostics.rs` — minimal ring buffer with capacity test.
- `pressure.rs` — injectable `PressureSource` trait is the right seam for tests.
- `run.rs` — argv parse and loop sleep ladder are appropriately thin; core owns `Runtime::tick`.
- `m2_integration.rs` — hog spawn + rate-limit merge gate is focused (does not claim to cover LowYield/Thrashing yet).
- Trim lock across settle in `measured_soft_trim` honors M1 lesson (lock spans full window).

### 7. Legibility (only if no larger issue)

`crates/ramjob-core/tests/m2_integration.rs:74-78` **nit**: test fabricates member `private_working_set_bytes` from working set when private WS lags — documents a real NtQSI warmup quirk; keep comment, consider shared test helper with gate integration.

`crates/ramjob-cli/src/run.rs:134-137` **nit**: sleep ladder duplicates SPEC poll intent already implied by FSM phase — acceptable for M2 CLI; later fold into `Runtime::recommended_sleep(phase)`.

---

## Approval bar (rubric)

| Gate | Status |
|------|--------|
| No clear structural regression | **Fail** — wrong exclusion policy; Thrashing/refault unwired; ineffective-trim contract violated; live ARM predicate unreachable |
| No obvious missed code-judo | **Fail** — duplicate §2.3 measurement vs gate; dead ineffective stub; double `fsm.step` orchestration |
| No unjustified 1k explosion | **Pass** |
| No spaghetti-growth / special-case tangle | **Fail** — silent compress fallback; ignored trim outcome; CLI pressure fallback branch |
| No hacky/magic abstraction | **Fail** — post-trim flags on pre-trim input struct; `assume_faults_when_low` dev flag never wired |
| No unnecessary wrapper/optionality churn | **Pass** (trait + pure FSM are earned) |
| No wrong-layer / duplicated helpers | **Fail** — measurement duplicated; config backup in wrong layer vs SPEC |
| Obvious decomposition opportunity | **Pass** size-wise; **shared trim-yield helper** is the decomposition that matters |

**Do not approve** until runtime wires correct FSM feedback (GF-based ineffective, refault detector), uses `ProtectInteractive`, and either implements live fault sampling or documents+tests the degraded ARM mode explicitly. Prefer extracting shared measurement from gate and deleting the dead `ineffective` stub.

---

## Compact finding list (`path:line severity problem. fix.`)

```
crates/ramjob-core/src/runtime.rs:154 blocker: daemon uses ExclusionPolicy::None; trims interactive/foreground. Use ProtectInteractive for runtime soft-trim.
crates/ramjob-core/src/runtime.rs:92 blocker: refault_hot always false; Thrashing FSM never reachable in production. Wire post-trim refault detect (>90% in 5s) into GroupFsmInput.
crates/ramjob-core/src/runtime.rs:113 blocker: trim_was_ineffective from ry_live<0.1 conflates LowYield with GF>0.9C ineffective trim; WouldBackstop counts wrong. Set from post-trim GF vs TRIM_TARGET_RATIO; keep ry_live for LowYield only.
crates/ramjob-core/src/pressure.rs:72 blocker: WinPressure reports faults=0; policy ARM (low&&faults>30) never fires live. Sample hard-fault rate or wire assume_faults_when_low + document degraded mode.
crates/ramjob-core/src/runtime.rs:136 important: measured_soft_trim duplicates gate::measure_under_lock. Extract shared measure_soft_trim_yield; single §2.3 owner.
crates/ramjob-core/src/runtime.rs:113 important: dead ineffective/gf_after stub (estimate_group_gf_after with empty procs). Delete or wire real post-sample GF.
crates/ramjob-core/src/config.rs:49 important: unknown version returns Err; SPEC requires config.bak backup + regenerate. Implement in load/ensure_config path.
crates/ramjob-core/src/runtime.rs:156 important: ignores soft_trim outcome; increments trims_attempted on no-op/rate-limited trim. Check outcome like gate require_real_trim.
crates/ramjob-core/src/runtime.rs:141 important: compress_store_ws unwrap_or(0) skews Ry_live on failure. Propagate Result; skip trim feedback on error.
crates/ramjob-cli/src/run.rs:96 important: WinPressure fail → hidden SimulatedPressure disarm branch. Single PressureSource enum + explicit diagnostic.
crates/ramjob-core/src/runtime.rs:97 important: double fsm.step with stale gf; post-trim flags easy to misuse. Split decide/observe_post_trim API.
crates/ramjob-core/src/fsm.rs:38 suggestion: GroupFsmInput mixes pre/post-trim fields without phase types. PostTrimObservation on second method.
crates/ramjob-core/src/runtime.rs:77 suggestion: clones config.groups every tick. Borrow instead.
crates/ramjob-core/tests/m2_integration.rs:74 nit: WS fallback hack for hog private WS warmup. Shared test helper comment ok.
```

---

## Fixes applied

**Status:** fixed 2026-07-27 (Task 9).

| Finding | Change |
|---|---|
| C1 ExclusionPolicy::None | Runtime uses `ProtectInteractive` via `run_gate_on_group` |
| C2 refault_hot stub | `apply_post_trim`: `gf1 >= 0.9 * gf0` |
| C3 ineffective from ry_live | `trim_was_ineffective` = `gf1 > 0.9C`; ry_live only for LowYield |
| C4 live ARM unreachable | `WinPressure::assume_faults_when_low = true` by default (windows-rs 0.58 has no fault field on `PERFORMANCE_INFORMATION`; real counter deferred) |
| I5 duplicate measure | Deleted runtime copy; calls `gate::run_gate_on_group` |
| I6 config.bak | `load_config_file` backs up + regenerates on unknown version |
| I7 trim accounting | Gate `require_real_trim`; runtime skips on Err, no rate bump |
| I8 compress unwrap | Handled inside gate (`Option` cs → optional ry_live) |
| I9 CLI silent fallback | Explicit `eprintln!` on WinPressure failure / degrade |
| I10 double step | `observe_post_trim` + `apply_post_trim` helper |

Verdict after fixes: Critical/Important addressed; live hard-fault/sec sampling remains documented degraded mode (not a blocker for M2 simulate-armed / hog verify).
