---
name: windows-msvc-sdk-env
description: Ensure MSVC + Windows SDK LIB/INCLUDE before cargo link on incomplete VS installs. Use when RamJob builds fail with LNK1181 kernel32.lib.
---

# Windows MSVC SDK env

Leading word: **link env**.

## When

`cargo build` / `cargo test` fails with `LNK1181: cannot open input file 'kernel32.lib'` or missing Windows Kits Lib/Include.

## Steps

1. Prepend `%USERPROFILE%\.cargo\bin` to PATH.
2. Prefer a full Windows 10/11 SDK via elevated VS Installer (Kits `Lib` + `Include` present).
3. If Kits are incomplete, point `LIB`/`INCLUDE` at a recovered SDK tree (e.g. NuGet `Microsoft.Windows.SDK.CPP` extracted under `%LOCALAPPDATA%\Temp\winsdk-nupkg\extracted`) plus MSVC toolset lib paths under BuildTools.
4. Re-run `cargo test --workspace` and record the env recipe in the task report if nonstandard.

**Done when:** `cargo test --workspace` links without LNK1181 on this machine.
