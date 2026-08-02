# M5 verify — ETW / adaptive / budget

**Branch:** `milestone/m5-etw-budget`  
**Date:** 2026-08-03

## Commands (intended)

```powershell
. .\scripts\dev-env.ps1
$env:CARGO_TARGET_DIR = "$env:USERPROFILE\ramjob-target"

cargo test -p ramjob-core discovery
cargo test -p ramjob-core adaptive
cargo test -p ramjob-core budget --release
cargo test -p ramjob-core
cargo build -p ramjob-cli -p ramjob-app
```

## Results on this host

### 2026-08-03 ~02:06 (SAC On)

| Check | Result |
|---|---|
| `SmartAppControlState` | **On** |
| Compile | OK |
| Run test binaries | **FAIL — os error 4551** |

### 2026-08-03 ~02:20 (SAC Off — task 56 thermo)

| Check | Result |
|---|---|
| `SmartAppControlState` | **Off** |
| `cargo test -p ramjob-core` | **151 passed**, 3 ignored; m2_integration + m4_backstop green |
| `cargo build -p ramjob-cli` | OK |
| `cargo build -p ramjob-app` | OK |

### 2026-08-03 ~02:12 (SAC Off — user)

| Check | Result |
|---|---|
| `SmartAppControlState` | **Off** |
| `cargo test -p ramjob-core` | **149 passed**, 3 ignored; m2_integration + m4_backstop green |
| `cargo test -p ramjob-core budget --release` | **2 passed** (WS ceiling ok) |
| `cargo build -p ramjob-cli` | OK |
| `cargo build -p ramjob-app` | **FAIL** until `DiscoverySource: Send` fix (AppState) — in flight |

## Budget harness (task 6)

- Module: `crates/ramjob-core/src/budget.rs`
- `sample_own_working_set_bytes()` via `GetProcessMemoryInfo`
- Ceiling assert: `IDLE_WS_CEILING_BYTES` = 25 MB (SPEC §6)
- Debug builds: `idle_runtime_working_set_within_ceiling` is `#[ignore]` (debug test WS exceeds ceiling); use `--release` when SAC allows execution

## Limitations

1. **SAC On** blocks unsigned cargo/rustup artifacts — see lesson `windows-smart-app-control-cargo`.
2. Turning SAC **Off** is one-way until OS reset; not flipped by agents.
3. Idle CPU % sampling deferred (noisy on short windows); WS ceiling is the CI assert.

## Resume when

SAC Off or path allowlisted → re-run the command block above; paste fresh results into this file.
