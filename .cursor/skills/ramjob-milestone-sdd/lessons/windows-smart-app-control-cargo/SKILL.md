---
name: windows-smart-app-control-cargo
description: >-
  Smart App Control (SAC) blocks unsigned cargo build scripts with os error 4551.
  Use when RamJob or other Rust builds fail with Application Control / LNK-adjacent
  "never executed" / error 4551 on Windows 11.
---

# Windows Smart App Control + cargo

Leading word: **SAC target dir**.

## When

`cargo build` / `cargo test` fails with:
- `An Application Control policy has blocked this file. (os error 4551)`
- build-script `(never executed)` for crates like `proc-macro2`, `serde`, `indexmap`
- Smart App Control state `On` (`Get-MpComputerStatus`.SmartAppControlState)

## Steps

1. Confirm SAC: `Get-MpComputerStatus | Select-Object SmartAppControlState` → `On`.
2. Do **not** build into a non-profile drive (`E:\…\target`) as the only output; SAC often blocks fresh unsigned build scripts there.
3. Before cargo, set a user-profile target and load MSVC env:

```powershell
. .\scripts\dev-env.ps1
$env:CARGO_TARGET_DIR = "$env:USERPROFILE\ramjob-target"
```

4. Prefer incremental rebuilds of already-trusted artifacts under that dir. A clean rebuild of large graphs (Tauri) may still trip SAC until the user turns SAC **Off** (Windows Security → App & browser control → Smart App Control; one-way until OS reset).
5. Re-run the failing package first (`cargo test -p ramjob-core`) before `--workspace`.
6. Record the recipe in the task report when nonstandard.

**Done when:** the intended `cargo` command links and runs without 4551, or the report states SAC Off is required for the remaining crates.
