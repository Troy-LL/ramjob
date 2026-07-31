# RamJob

Windows tray utility that caps application memory by install-root group (Opera GX–style limiter, generalized).

**Status:** M0–M2 ship as the Rust CLI (`list` / `gate` / `run`). M3 tray + instrument panel lives in `ramjob-app` (Tauri); source is on `main`, but a live tray build on this host still needs Smart App Control to allow unsigned cargo build scripts. M4+ (Job Object hard backstop, etc.) not started.

## Requirements

- Windows 10 1809+ / Windows 11 (x64)
- Rust stable (MSVC) + Visual Studio Build Tools / Windows SDK

```powershell
. .\scripts\dev-env.ps1
cargo test --workspace
cargo run -p ramjob-cli -- list
```

## Docs

- [SPEC.md](SPEC.md) — product truth (v0.3)
- Agent workflow: `.cursor/skills/ramjob-milestone-sdd/`

## License

All rights reserved until a license is chosen.
