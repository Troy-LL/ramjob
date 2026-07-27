# M2 verify — policy FSM + pressure

Date: 2026-07-27  
Branch: `milestone/m2-policy-fsm`

## Automated (synthetic hog)

```powershell
. .\scripts\dev-env.ps1
$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
cargo build -p ramjob-hog
cargo test -p ramjob-core --test m2_integration
cargo test --workspace
```

| Check | Result |
|---|---|
| Cap below hog GF + `SystemArm::Armed` → `trims_attempted >= 1` | **Pass** |
| Second tick within 20s → `trims_attempted == 0` (rate limit) | **Pass** |
| `cargo test --workspace` | See commit CI / local run |

Test: `crates/ramjob-core/tests/m2_integration.rs`  
(`armed_over_cap_trims_once_then_rate_limits`)

## CLI smoke (simulate-armed)

```powershell
$tmp = Join-Path $env:TEMP "ramjob-m2-test.toml"
@"
version = 2
runaway_multiplier = 3.0
"@ | Set-Content $tmp
cargo run -p ramjob-cli -- run --once --simulate-armed --config $tmp
```

| Check | Result |
|---|---|
| `ramjob run --once --simulate-armed` exits 0, prints `system=Armed` | **Pass** |

## Live LowMemory smoke

**Skipped** — no forced system LowMemoryResourceNotification in this verify pass.  
Win pressure path covered by unit tests (`SimulatedPressure` → policy arm) and `WinPressure::new` compile/link; live ARM dwell requires real memory pressure or `assume_faults_when_low` demo override.

## Notes

- Soft trim under measured path holds the global trim lock for the 3s settle (M1 lesson).
- Job Objects / tray UI intentionally out of scope (M4 / M3).
