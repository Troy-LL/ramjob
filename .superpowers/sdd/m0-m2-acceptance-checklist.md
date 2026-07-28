# M0–M2 acceptance checklist (what we want to see)

Date: 2026-07-27  
Branch: `milestone/m2-policy-fsm`  
Source of truth: `SPEC.md` §9–§10 + M2 design

Legend: **Pass** / **Partial** / **Fail** / **Skip** / **N/A (later milestone)**

---

## Automated (re-run 2026-07-27)

| Want to see | How | Result |
|---|---|---|
| Workspace tests green | `cargo test --workspace` | **Pass** |
| M2 hog SoftTrim once + rate-limit | `cargo test -p ramjob-core --test m2_integration` | **Pass** |
| CLI help exposes list / gate / run | `ramjob --help` | **Pass** |
| Live enumerate + group + GF | `ramjob list` (non-empty ranked rows) | **Pass** |
| Daemon tick Armed (sim) | `ramjob run --once --simulate-armed` | **Pass** (`system=Armed`) |

---

## M0 — group → GF

| Want to see | SPEC | Result |
|---|---|---|
| Enumerate processes (NtQSI) | §5.1 | **Pass** (code + `list`) |
| Group by install-root / image | §5.3 | **Pass** (heuristic; live Brave/Discord/Cursor keys look sane) |
| Print GF (private WS sum) | §3.1 / M0 | **Pass** |
| Formal corpus ≥95% + zero cross-app merges on 3 machines | §10 M0 criterion | **Partial** — machine 1 only: Brave+Discord 25/25 correct, 0 merges; full corpus + 2 more machines still required. See `m0-corpus-machine1.md` |
| Filter noise / denylist | §5.2 | **Pass** (code path; not re-audited end-to-end here) |

---

## M1 — soft trim + compression gate

| Want to see | SPEC | Result |
|---|---|---|
| Soft trim EmptyWorkingSet, largest private WS first | §4.2 / §2.3 | **Pass** |
| Foreground / visible exclusion on daemon path | §4.2 | **Pass** (ProtectInteractive in runtime) |
| Trim lock covers settle (3s) | §2.3 | **Pass** (gate/runtime shared owner) |
| Synthetic hog Ry_bench Pass (≥0.5) | §9.2 | **Pass** (prior M1 evidence; not re-gated this session) |
| Real-app gates (Brave/Discord/Spotify/VS Code) | §9.2 | **Pass** (local notes: Pass; files untracked) |
| Record Ry_live alongside Ry_bench | §2.3 / §9.2 | **Pass** |
| Runtime uses Ry_live cutoff (placeholder 0.35) | §4.2 | **Pass** (wired; cutoff still placeholder) |
| §9.3 pivot if gate Fail | §9.3 | **N/A** — gate treated Pass |

---

## M2 — policy FSM + pressure

| Want to see | SPEC / design | Result |
|---|---|---|
| config.toml v2 + caps + runaway_multiplier | §8.3 / §4.1 | **Pass** |
| Unknown version → config.bak + regenerate | §8.3 | **Pass** (unit test) |
| System Armed/Disarmed + dwell | §4.1 | **Pass** (policy unit tests + sim) |
| Live LowMemory + hard-fault confirm ARM | §4.1 | **Partial** — notifications live; faults via `assume_faults_when_low` (no real fault counter) |
| Per-group Idle/Pressure/Trim/LowYield/Thrashing | §4.2 | **Pass** (FSM unit tests; Thrashing needs live refault_hot from measure) |
| WouldBackstop telemetry only (no Job Object) | §4.2 / M4 | **Pass** |
| Runaway force-arm while Disarmed | §4.1 | **Pass** (FSM; not live-smoked this session) |
| `ramjob run` loop | M2 design | **Pass** |
| Synthetic verify: over-cap + Armed → trim, then rate-limit | M2 plan Task 8 | **Pass** |
| Live LowMemory smoke | M2 verify optional | **Skip** |

---

## Explicitly not yet (do not expect)

| Item | Milestone |
|---|---|
| Tray / panel / sliders | M3 |
| Job Object hard backstop | M4 |
| ETW / budget CI | M5 |
| Autostart / first-run polish | M6 |
| §6 idle CPU/WS CI asserts | M5 / §9.4 |

---

## Gaps to decide before M3

1. ~~Formal M0 corpus~~ — **Decided 2026-07-27:** proceed to M3; full corpus stays user testing (SPEC §10).
2. Promote `assume_faults_when_low` to a real hard-fault/sec sample, or accept degraded ARM for M3?
3. Lock `Ry_live` cutoff from M1 paired numbers, or keep 0.35 placeholder into M3?
4. Commit/track M1 gate markdown notes (currently untracked)?

---

## Ponytail (complexity only)

See companion section in session notes / below; ~280 lines possible cut, no correctness claims.
