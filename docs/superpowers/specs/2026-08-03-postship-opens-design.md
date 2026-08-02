# RamJob post-ship OPENs — design

**Date:** 2026-08-03  
**Status:** Locked (user: “do everything”)  
**Amends:** SPEC §4.2, §5.2–5.3 notes, §6.1 battery OPEN, §11 OPENs 1/3/4/5/6/7

## Decisions

| OPEN | Decision |
|---|---|
| Battery (§6.1) | Keep pressure gating; **raise soft-trim rate limit** on AC→battery (`TRIM_RATE_LIMIT` 20s → **60s** on battery). Do not sleep the whole engine (always-on cadence stays). |
| Chromium auto-backstop (§4.2) | Bundled path/exe family match; when user **sets a cap** on a Chromium-family group, default `always_enforce = true` (still shows §7.4 warning when enabling via UI; auto-set may skip extra modal if already warned once — keep existing set_flags warning path). User can turn off via ⚙. |
| Distribution | **Portable release exe** + HKCU Run (shipped M6). No MSI/MSIX in-tree. |
| Code signing | Human-gated EV/OV cert; provide `scripts/sign-release.ps1` that no-ops/errors clearly without cert. |
| explorer.exe | **Never** manage (hard denylist stays). |
| VS Code sub-grouping | **One group** (current heuristic); no split. |

## Non-goals

- Buying/installing a code-signing cert
- MSI packaging
- Sleeping Runtime entirely on battery
- Silent `always_enforce` with zero user cap interaction

## Verify

- Unit: battery rate limit selection; chromium family match; set_cap pins always_enforce for chrome path
- `cargo test -p ramjob-core`; `cargo build -p ramjob-app`
