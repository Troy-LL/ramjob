# M0 corpus gate — machine 1 (this PC)

**Date:** 2026-07-27  
**Machine:** Troy laptop (single machine; SPEC requires **three**)  
**Build:** `milestone/m2-policy-fsm` (`ramjob list` + member dump via temp `m0_dump`)  
**SPEC:** §10 — `pass = (correct/total ≥ 0.95) && (cross_app_merges == 0)`

## Corpus presence

| App | Running? | Visible in RamJob (≥50 MiB GF)? |
|---|---|---|
| Brave | Yes (~18–20 procs) | Yes — `c:\users\troyl\appdata\local\bravesoftware` |
| Chrome | **No** | — |
| VS Code | **No** (Cursor is present; not a SPEC substitute) | — |
| Discord | Yes (6 procs) | Yes — `c:\users\troyl\appdata\local\discord` |
| Slack | **No** | — |
| Spotify | Launcher only (~14 MiB WS) | **No** (under GF floor / not full client) |
| Steam client | **No** | — |
| Teams | **No** | — |

## Hand labels (running corpus members)

### Brave → expected group: BraveSoftware install root

All 19 members under `…\bravesoftware` were `brave.exe` or `BraveCrashHandler*.exe` under `BraveSoftware\…`.  
**Assignments:** 19/19 correct. **Cross-app merge:** none.

### Discord → expected group: Discord install root (version segment stripped)

All 6 members under `…\discord` were `Discord.exe` under `app-1.0.9249` (version strip worked).  
**Assignments:** 6/6 correct. **Cross-app merge:** none.

### Score on this machine (present corpus only)

| Metric | Value |
|---|---|
| Labeled processes | 25 (19 Brave + 6 Discord) |
| Correct | 25 |
| Accuracy | **100%** (≥ 0.95) |
| Cross-app merges (Brave↔Discord) | **0** |

**This-machine slice:** Pass for apps that were actually running.

## Formal SPEC gate

| Requirement | Status |
|---|---|
| Full corpus (8 apps) | **Fail / incomplete** — 5 apps not running; Spotify not visible |
| Three machines | **Fail / incomplete** — only machine 1 |
| Overall formal Pass | **Not achieved** |

## Non-corpus observations (not scored)

- **Cursor** correctly grouped under `…\programs\cursor` (incl. helper `node.exe`).
- **`image:claude`** merges `.local\bin\claude.exe` and Cursor-extension `claude.exe` via image-stem fallback — related tools, different install roots; watch for over-merge if treated as distinct apps.
- **`image:cmd` / `image:powershell`** pull assorted `node.exe` via runtime-host PPID walk — noisy, not corpus.

## What you need for a formal Pass

1. Launch full corpus: Chrome, VS Code, Slack, Spotify (main client), Steam, Teams (plus Brave/Discord already OK).
2. Re-run dump + label on this machine → fill gaps.
3. Repeat on **two more PCs**.
4. Aggregate: ≥95% correct, **zero** cross-app merges everywhere.
5. Update this file (or add `m0-corpus-machine-2/3.md`) and mark SPEC M0 Pass only then.

## Raw `ramjob list` (same session)

```
c:\users\troyl\appdata\local\bravesoftware   … ~1 GiB class
c:\users\troyl\appdata\local\programs\cursor …
image:claude …
c:\users\troyl\appdata\local\discord …
```
