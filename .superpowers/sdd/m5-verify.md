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

## Results on this host (2026-08-03 ~02:06)

| Check | Result |
|---|---|
| `SmartAppControlState` | **On** |
| Compile `ramjob-core` | OK (links under `%USERPROFILE%\ramjob-target`) |
| Run any `ramjob_core-*.exe` test binary | **FAIL — os error 4551** Application Control blocked |
| Windows toast | “Part of this app has been blocked — Some features of `C:\Users\admin\.rustup`” (+14) |

Earlier in this milestone (before SAC tightened), discovery/adaptive/runtime tests passed (153 lib + discovery suite). Those results stand as historical evidence on commits through `f54e9ca`. **Re-execution is currently impossible** without SAC Off or an allowlist for `%USERPROFILE%\ramjob-target` and `.rustup`.

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
