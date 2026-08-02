# RamJob

Windows tray utility that caps application memory by install-root group (Opera GX–style limiter, generalized).

**Status:** M0–M5 shipped (CLI + tray panel, Job Object backstop, ETW/adaptive/budget). **M6** adds config autostart, startup preflight, and first-run panel copy — shippable via local release build (no MSI).

## Requirements

- Windows 10 1809+ / Windows 11 (x64)
- Rust stable (MSVC) + Visual Studio Build Tools / Windows SDK
- WebView2 runtime (usually preinstalled on Windows 11)

## Quick start — tray app (M6 ship path)

From repo root:

```powershell
. .\scripts\dev-env.ps1
$env:CARGO_TARGET_DIR = "$env:USERPROFILE\ramjob-target"

cargo build -p ramjob-app --release
& "$env:CARGO_TARGET_DIR\release\ramjob-app.exe"
```

Tray icon appears; click to open the panel. Config: `%APPDATA%\RamJob\config.toml`.

**No MSI** — portable `ramjob-app.exe` only. Copy manually if needed.

**Signing (human-gated):** SmartScreen/AV will distrust unsigned binaries that open other processes. After you have an EV/OV cert, build release then run `.\scripts\sign-release.ps1` with `$env:RAMJOB_SIGN_CERT` set (thumbprint or `.pfx` path).

If `cargo` fails with os error 4551 (Smart App Control), keep `CARGO_TARGET_DIR` under your user profile as above, or turn SAC Off in Windows Security (one-way until OS reset).

## Signing (release)

EV/OV code signing is human-gated. After a release build, sign with:

```powershell
. .\scripts\dev-env.ps1
$env:CARGO_TARGET_DIR = "$env:USERPROFILE\ramjob-target"
$env:RAMJOB_SIGN_CERT = "<thumbprint-or-pfx-path>"
.\scripts\sign-release.ps1
```

`RAMJOB_SIGN_CERT` must be set (40-char SHA1 thumbprint or path to `.pfx`; optional `RAMJOB_SIGN_CERT_PASSWORD` for PFX). Without it, the script exits non-zero with a clear message.

## Quick start — CLI

```powershell
. .\scripts\dev-env.ps1
cargo test --workspace
cargo run -p ramjob-cli -- list
```

## Verify

M6 checklist and test results: [.superpowers/sdd/m6-verify.md](.superpowers/sdd/m6-verify.md)

## Docs

- [SPEC.md](SPEC.md) — product truth (v0.3)
- Agent workflow: `.cursor/skills/ramjob-milestone-sdd/`

## License

All rights reserved until a license is chosen.
