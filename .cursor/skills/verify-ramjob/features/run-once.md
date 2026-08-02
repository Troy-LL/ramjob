# Policy run once

Run executes one policy tick (or a loop when not `--once`): pressure state, optional soft trims for over-cap groups, and a `tick system=… trims=…` status line. Verify uses a disposable config and `--simulate-armed` so the machine need not be under real memory pressure.

## Sub-features

- `run-once` performs a single tick then exits.
- `run-simulate` forces Armed via `--simulate-armed` (CLI-only).
- `run-config` loads caps/flags from `--config` instead of AppData.

## How to get to it (user POV)

- Run `ramjob run` for the continuous daemon, or `ramjob run --once` for a single tick.
- Optional: `--config <path>`, `--simulate-armed`.

## Driving it with verify-ramjob

Preconditions:

- Doctor OK.
- Do not have a long-lived `ramjob run` or `ramjob-app` enforcement session racing the same PIDs unless intentional.

- **Drive.** Run `.\.cursor\skills\verify-ramjob\helpers\verify-ramjob.ps1 run-once -EvidenceDir <evidence>`. Exit code `0`.
- **Observe.** `<evidence>\run.stdout.txt` contains `tick system=`. `<evidence>\config.verify.toml` is the config that was passed (version template; no AppData write required).
- **Proof.** meta records feature `run-once` and the exact argv. Retain stdout after cleanup.

## Gotchas

- Without `--simulate-armed`, an Idle/Disarmed machine may trim nothing — that is valid but weak proof of the trim path; prefer simulate for CI-like verify.
- Config is loaded once at process start; editing AppData while a daemon runs will not reload.
- Never point verify at the user’s real AppData config for destructive edits.
