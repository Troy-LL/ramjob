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

**No MSI** — M6 documents a local `cargo build --release` binary only. Copy `ramjob-app.exe` manually if you need it on another machine; there is no signed installer yet.

If `cargo` fails with os error 4551 (Smart App Control), keep `CARGO_TARGET_DIR` under your user profile as above, or turn SAC Off in Windows Security (one-way until OS reset).

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
