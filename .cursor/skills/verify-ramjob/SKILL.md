---
name: verify-ramjob
description: >-
  Drive RamJob the way a user does — primarily the Windows CLI (`ramjob list|gate|run`),
  with optional tray-panel notes. Use when proving a change, after M0–M3 work, or when an
  agent needs scripted evidence that enumerate/gate/policy behavior still works.
---

# Verify RamJob

Primary surface: **CLI** (`ramjob`). Secondary: Tauri tray panel (`ramjob-app`) — GUI-only; do not claim panel proof from headless runs. Harness-only: `ramjob-hog`.

Evidence root (survives cleanup): `.cursor/skills/verify-ramjob/artifacts/<run-id>/`  
Feature recipes: `features/` (read the index, then the feature file).

## Launch

From repo root on Windows (x64):

```powershell
. .\scripts\dev-env.ps1
# Smart App Control on this machine blocks unsigned build scripts under E:\ —
# keep artifacts under the user profile when cargo fails with os error 4551:
$env:CARGO_TARGET_DIR = "$env:USERPROFILE\ramjob-target"

$RunId = Get-Date -Format "yyyyMMdd-HHmmss"
$Evidence = Join-Path $PSScriptRoot "artifacts\$RunId"
New-Item -ItemType Directory -Force -Path $Evidence | Out-Null

# Build once per verify session (CLI + hog). Do not launch ramjob-app unless verifying panel features.
cargo build -p ramjob-cli -p ramjob-hog
```

**Ready when:** `Get-Command` can resolve the built `ramjob.exe` under `$env:CARGO_TARGET_DIR\debug\ramjob.exe` (or `.\target\debug\ramjob.exe` if target dir is default) and `.\.cursor\skills\verify-ramjob\helpers\verify-ramjob.ps1 doctor` exits 0.

**Teardown:** CLI commands are short-lived — no server. Kill only hog/daemon processes **this run started** (PIDs recorded under `$Evidence\pids.txt`). Never `Stop-Process -Name ramjob`.

Helper wrapper (preferred):

```powershell
$Verify = ".\.cursor\skills\verify-ramjob\helpers\verify-ramjob.ps1"
& $Verify doctor
& $Verify list -EvidenceDir $Evidence
```

## Doctor

Read-only health check — run first whenever anything looks off:

```powershell
.\.cursor\skills\verify-ramjob\helpers\verify-ramjob.ps1 doctor
```

Expect exit 0 and stdout including: `cargo=ok`, `ramjob=ok`, `msvc_link=ok` (or `msvc_link=missing` with a clear fail), and the resolved `ramjob` path. Fail if `ramjob --help`-style bad args return exit ≠ 2 when invoked as `ramjob` with no valid subcommand path — doctor uses `ramjob list` dry health via a one-shot list that must exit 0.

Never drive an instance or config path this run did not create when mutating (`run` / panel). `list` and `gate` against live processes are OK.

## Drive

1. Open `features/README.md`, pick a feature ID.
2. Follow that file’s **Driving it with verify-ramjob** section literally.
3. Prefer CLI over GUI. For panel features, require an interactive Windows desktop session and document that constraint in the proof.

Stable handles:

| Surface | Handle |
|---------|--------|
| CLI list | Tab-separated stdout: `group_key\tmembers\thuman_gf` |
| CLI gate | Lines `Ry_bench:`, `Ry_live:`, `Classification:` |
| CLI run | Line `tick system=… trims=…` |
| Config | Temp TOML via `ramjob run --config <path>` only |
| Panel | Window title `RamJob`; DOM `#state-pill`, `#app-grid`, `#pause-all-btn` |

## Evidence

Proof standards:

- Exercise the real user path (`ramjob list|gate|run` or the tray UI), not internal test-only setters alone.
- Capture the **command** (or UI action) and the **resulting state** (stdout file, config TOML, gate markdown).
- Side effects: gate `--out` file exists; `run --config` writes only the temp config you passed; hog process exited or was killed by this run.
- Unit tests (`cargo test -p …`) support confidence but do not replace a mapped feature drive unless the feature file says the test is the proof.

Write under `$Evidence`:

| Artifact | Content |
|----------|---------|
| `meta.txt` | run-id, feature ID, git HEAD, commands |
| `list.stdout.txt` / `gate.stdout.txt` / `run.stdout.txt` | Full CLI capture |
| `gate-out.md` | Gate markdown when using `--out` |
| `config.verify.toml` | Disposable config for `run` proofs |
| `pids.txt` | Child PIDs started this run |

## Cleanup

```powershell
.\.cursor\skills\verify-ramjob\helpers\verify-ramjob.ps1 cleanup -EvidenceDir $Evidence
```

Stops PIDs listed in `$Evidence\pids.txt` only. Deletes `$Evidence\scratch\` if present. **Does not** delete stdout/markdown/meta proof files. Does not touch `%APPDATA%\RamJob\config.toml` unless the feature recipe created a dedicated backup and says to restore it — prefer `--config` temp files instead.

## Helpers

| Invocation | Purpose |
|------------|---------|
| `helpers\verify-ramjob.ps1 doctor` | Env + binary health |
| `helpers\verify-ramjob.ps1 list -EvidenceDir <dir>` | Run `list`, save stdout, validate rows |
| `helpers\verify-ramjob.ps1 gate -EvidenceDir <dir> [-Mb 64]` | Hog + `gate --image ramjob-hog` |
| `helpers\verify-ramjob.ps1 run-once -EvidenceDir <dir>` | Temp config + `run --once --simulate-armed` |
| `helpers\verify-ramjob.ps1 cleanup -EvidenceDir <dir>` | Kill recorded PIDs; keep proof |

All helpers must be invoked from repo root after `dev-env.ps1` (or they source it).

## Isolation

- Prefer `ramjob run --config <temp.toml>` — never point verify mutators at the user’s live `%APPDATA%\RamJob\config.toml` unless the feature file explicitly requires it.
- Do not run `ramjob run` (daemon) and `ramjob-app` together expecting one truth.
- `ramjob-app` has no `--config` flag; panel verify shares AppData — refuse concurrent panel drives against the user’s session; say so and stop.
- No tray singleton: two `ramjob-app` processes fight the same config.

## Known gaps

- README status line may lag milestones (tray exists in-tree).
- Job Objects not shipped; `always_enforce` is FSM-only.
- No Tauri `--simulate-armed`; Armed/Warning pill colors need live pressure or CLI daemon.
- Smart App Control (os error 4551): use `$env:CARGO_TARGET_DIR = "$env:USERPROFILE\ramjob-target"`.
