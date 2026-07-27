# RamJob

Windows tray utility that caps application memory by install-root group (Opera GX–style limiter, generalized).

**Status:** M0 (enumerate → group → GF) and M1 (soft trim + compression gate) are implemented as a Rust CLI. Tray/UI is not started until later milestones.

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
