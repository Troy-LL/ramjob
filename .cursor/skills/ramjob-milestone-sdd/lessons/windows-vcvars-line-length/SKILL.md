---
name: windows-vcvars-line-length
description: >-
  Re-sourcing scripts/dev-env.ps1 (vcvars64) in the same PowerShell session
  prints "The input line is too long" / "syntax of the command is incorrect".
  Use when RamJob cargo still works but vcvars noise appears, or env setup fails mid-session.
---

# Windows vcvars line length

Leading word: **fresh vcvars shell**.

## When

After calling `. .\scripts\dev-env.ps1` more than once in one shell, stdout shows:
- `The input line is too long.`
- `The syntax of the command is incorrect.`

Cargo may still find `link.exe` from a prior successful load.

## Steps

1. Prefer a **new** PowerShell process for each verify/build session instead of nesting vcvars.
2. Or call `dev-env.ps1` only when `Get-Command link.exe` fails.
3. Do not append vcvars output into an already-expanded `PATH`/`INCLUDE`/`LIB` repeatedly.
4. Optional hardening (separate PR): make `dev-env.ps1` no-op when `link.exe` already resolves.

**Done when:** a clean shell sources `dev-env.ps1` once with no line-length error, or the script skips when already loaded.
