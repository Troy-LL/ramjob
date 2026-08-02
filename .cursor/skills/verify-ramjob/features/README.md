# RamJob verification map

This directory is the maintained source for verifying user-facing RamJob behavior. Read this index before driving the app, then use the matching feature file as the recipe.

## Baseline preconditions

- Repo root: Windows x64 with Rust MSVC + VS Build Tools.
- Run `. .\scripts\dev-env.ps1` before cargo.
- If builds fail with Application Control / os error 4551, set `$env:CARGO_TARGET_DIR = "$env:USERPROFILE\ramjob-target"`.
- Prefer CLI proofs. Do not drive the user’s live tray session for mutations.
- Production panel requires `app.withGlobalTauri: true` in `tauri.conf.json` so `window.__TAURI__` exists; without it the UI silently uses `MOCK_SNAPSHOT`.
- Put evidence under `.cursor/skills/verify-ramjob/artifacts/<run-id>/`.
- Run `helpers\verify-ramjob.ps1 doctor` and require exit 0 before drives.
- Never kill processes by name; only PIDs recorded for this run.

## Driving conventions

- Start every recipe from baseline unless its preconditions say otherwise.
- Treat every command as literal.
- Route CLI drives through `helpers\verify-ramjob.ps1` when a recipe names it.
- Restore or ignore AppData config: use `--config` temp files for `run`.
- Cleanup removes scratch/PIDs only — proof artifacts stay.

## Proof and skip reporting

- Capture the user action (CLI argv) and resulting state (stdout, `--out` file, temp config).
- CLI proof includes command, stdout, stderr, and exit code.
- Mutation proof includes a second read of the written file when applicable.
- Record feature ID and entry point in `meta.txt`.
- Report an unreachable path with the attempted command and unmet precondition.
- Do not report a skipped entry point as verified through a different path.
- Panel/GUI paths require an interactive desktop; headless agents must mark them unreachable, not “passed via unit test,” unless the feature file allows that substitute.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing the user-visible behavior. It then uses exactly four H2 sections in this order:

1. `Sub-features`
2. `How to get to it (user POV)`
3. `Driving it with verify-ramjob`
4. `Gotchas`

## Features

- [List group footprint](./list-gf.md) — enumerate → group → GF table (M0).
- [Compression gate](./gate-ry.md) — soft-trim yield classification with hog (M1).
- [Policy run once](./run-once.md) — armed tick + optional trim with disposable config (M2).
- [Tray panel](./tray-panel.md) — open panel, read Idle/Armed state (M3; GUI).
