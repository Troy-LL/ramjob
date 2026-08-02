# M6 verify — config, autostart, preflight, first-run (shippable)

**Branch:** `milestone/m6-shippable`  
**Date:** 2026-08-03  
**Head:** `35aca08` (tasks 1–5 through `4aa0114`)

M6 delivers HKCU Run autostart, startup §5.4 preflight, and §7.3 first-run panel copy. No MSI or signed installer in this milestone — ship via `cargo build -p ramjob-app --release` and run the binary locally.

## Environment

From repo root on Windows x64:

```powershell
. .\scripts\dev-env.ps1
$env:CARGO_TARGET_DIR = "$env:USERPROFILE\ramjob-target"
```

- **`scripts/dev-env.ps1`** — MSVC `link.exe`, Windows SDK libs, cargo on PATH. Required when the default toolchain cannot link.
- **`CARGO_TARGET_DIR`** — keeps build artifacts under the user profile. Use when Smart App Control blocks unsigned cargo/rustup binaries under repo paths (os error 4551). See lesson `windows-smart-app-control-cargo`.
- **SAC Off** — M5 verify required SAC Off on this host; agents do not flip SAC. Re-check with `Get-Process -Name smartscreen -ErrorAction SilentlyContinue` or Windows Security → App & browser control.

## Build and run (no MSI)

```powershell
cargo build -p ramjob-app --release
& "$env:CARGO_TARGET_DIR\release\ramjob-app.exe"
```

Tray icon appears; click to open the 420×600 panel. Config lives at `%APPDATA%\RamJob\config.toml`. There is **no** MSI, Store package, or code-signed installer in M6 — copy `ramjob-app.exe` (and any WebView2 runtime dependency on the target machine) manually if needed.

CLI (`ramjob-cli`) remains available for headless verify:

```powershell
cargo build -p ramjob-cli --release
& "$env:CARGO_TARGET_DIR\release\ramjob.exe" list
```

## Automated tests

```powershell
cargo test --workspace
cargo test -p ramjob-core autostart
cargo test -p ramjob-core preflight
cargo test -p ramjob-core first_run
cargo test -p ramjob-app
```

Optional budget ceiling (release only; debug WS exceeds 25 MB):

```powershell
cargo test -p ramjob-core budget --release
```

### Results on this host (2026-08-03, SAC Off)

| Check | Result |
|---|---|
| `cargo test --workspace` | **203 passed**, 3 ignored, 0 failed |
| `ramjob-core` unit + integration | 177 + 1 m2 + 4 m4 = **182** passed, 3 ignored |
| `ramjob-app` | **5** passed |
| `ramjob-cli` | **9** passed |
| `ramjob-hog` | **7** passed |
| `cargo build -p ramjob-app --release` | OK (~2m47s) |
| Release binary | `%USERPROFILE%\ramjob-target\release\ramjob-app.exe` |

## Manual verify checklist

### 1. Autostart toggle (tasks 1–2, 4)

| Step | Expected |
|---|---|
| Fresh install / delete `%APPDATA%\RamJob\config.toml` | `autostart = false` in new config |
| Launch `ramjob-app`; tray → **Settings** → **Start with Windows** | Unchecked |
| Enable toggle | `config.toml` has `autostart = true`; HKCU `...\Run` value `RamJob` present (quoted exe path) |
| Disable toggle | `autostart = false`; Run value removed |
| Restart app with `autostart = true` in config | Run key present without re-toggling |
| Stale group prune | Groups with `last_seen_unix` &gt; 90 days and not `pinned` removed on startup; save-back if pruned |

Registry path: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value name `RamJob`.

Unit coverage: `ramjob_core::autostart` (enable/disable/idempotent), `ramjob-app` `set_autostart` / startup sync tests.

### 2. Startup preflight (task 3)

| Probe | Panel / diagnostics |
|---|---|
| Pagefile disabled or &lt; 1 GB | Warning in diagnostics + `panel_notes` when applicable |
| Total RAM ≥ 32 GB | Dormancy note (trim may be rare) |
| Not elevated | Privilege note (some processes uncappable) |
| Once per process | `preflight::run_once()` cached; pushed to diagnostics ring on app init |

Verify: tray → info popover → **Copy diagnostics** includes `--- startup preflight (§5.4) ---` header and note lines.

Unit coverage: `ramjob_core::preflight` (pagefile, dormancy, privilege, `push_to_diagnostics`, `panel_notes`).

### 3. First-run panel (task 5)

| Condition | UI |
|---|---|
| No per-app caps (`cap_bytes > 0`) in config | `first_run = true`; `#first-run-hint` visible |
| Static explainer | “Set a cap on any app below…” (§7.3 one-liner, no wizard) |
| `preflight_notes` non-empty | Second paragraph in `#preflight-notes` while `first_run` |
| Any cap set | Hint hidden; normal app grid |

Unit coverage: `panel::build_snapshot_first_run_when_no_caps`, `build_snapshot_not_first_run_when_cap_set`, `preflight::panel_notes_*`.

### 4. Tray smoke (M3 baseline, unchanged)

- Panel opens from tray click; Pause all / Quit work.
- Per-app cap drag persists to `config.toml`.
- Copy diagnostics returns clipboard text.

GUI items require an interactive desktop session — not proven headless.

## Shippable definition (M6)

User can:

1. Build and run `ramjob-app` from source (release binary).
2. See honest §5.4 preflight in diagnostics and first-run copy.
3. Set a per-app cap from the panel.
4. Optionally enable **Start with Windows** (default off).
5. Leave RamJob resident in the tray.

Out of scope: MSI, elevated helper service, Store signing, Chromium auto-backstop.

## Related docs

- Design: `docs/superpowers/specs/2026-08-03-m6-shippable-design.md`
- Plan: `docs/superpowers/plans/2026-08-03-m6-shippable.md`
- Prior verify: `.superpowers/sdd/m5-verify.md`, `.superpowers/sdd/m3-verify.md`
- CLI harness: `.cursor/skills/verify-ramjob/SKILL.md`
