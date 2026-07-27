# 01 — Rust workspace scaffold

**Milestone:** M0

**What to build:** A buildable Rust workspace named `ramjob` with a CLI binary crate and library crates ready for scanner/grouper/accountant. `cargo test` and `cargo build` succeed on Windows MSVC.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] Workspace `Cargo.toml` with members `crates/ramjob-core`, `crates/ramjob-cli`
- [ ] `ramjob-core` is a `lib` crate (edition 2021) with empty modules `scanner`, `grouper`, `accountant` stubbed
- [ ] `ramjob-cli` depends on `ramjob-core`, binary name `ramjob`, `--help` prints usage
- [ ] `windows` crate dependency present on `ramjob-core` (features minimal for later NtQSI)
- [ ] `.gitignore` covers `target/`, `.superpowers/sdd/*-brief.md` reports ok to keep ledger

**Verify:** `$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path; cargo test --workspace`

**Notes:** No Tauri. No commits unless dispatch says so. Keep crates tiny.
