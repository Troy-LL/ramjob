# Task 9 fix report — M2 thermo Critical/Important

## Critical

| # | Finding | Status |
|---|---|---|
| C1 | Runtime `ExclusionPolicy::None` | **done** — `ProtectInteractive` |
| C2 | `refault_hot` always false | **done** — from `gf1 >= 0.9 * gf0` |
| C3 | `trim_was_ineffective` from `ry_live < 0.1` | **done** — from `gf1 > 0.9C` |
| C4 | WinPressure faults=0, live ARM dead | **done (degraded)** — `assume_faults_when_low=true` default; no live counter in windows 0.58 `PERFORMANCE_INFORMATION` |

## Important

| # | Finding | Status |
|---|---|---|
| I5 | Duplicate `measured_soft_trim` | **done** — `run_gate_on_group` |
| I6 | No `config.bak` on unknown version | **done** |
| I7 | Count trim on no-op | **done** — fail-closed via gate |
| I8 | `compress_store_ws` unwrap_or(0) | **done** — gate Option path |
| I9 | Silent SimulatedPressure fallback | **done** — explicit eprintln |
| I10 | Double `fsm.step` fragility | **done** — `observe_post_trim` / `apply_post_trim` |

## Suggestions

Deferred (not required for thermo clear): typed DiagnosticEvent, PostTrimObservation type on FSM API.

## Lesson

`single-measure-owner` under `.cursor/skills/ramjob-milestone-sdd/lessons/`.
