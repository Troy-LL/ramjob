Folded into SPEC.md v0.3 on 2026-07-27.

# RamJob — Spec Gap Resolutions (amendment to SPEC.md v0.2)

**Date:** 2026-07-27
**Status:** Approved. Folds into SPEC.md v0.3.
**Scope:** Twelve gaps found in v0.2. Each resolution below names the section of SPEC.md it
replaces or amends.

---

## 1. Measurement

*Replaces §2.3. Amends §4.1, §4.2, §9.2, §6.1.*

### 1.1 Two yield metrics

v0.2 defined a single `Ry` as `Δ(Available MBytes) / Δ(GF)` and used it in two places with
incompatible requirements: an offline go/no-go gate and a runtime policy check. `Δ Available`
is a system-wide quantity; attributing it to one group's trim is valid only on a quiesced
machine, and the runtime check by definition runs on a busy one.

Split into two named metrics:

```
Ry_bench = Δ(Available MBytes) / Δ(GF)          # M1 gate only, quiesced machine
Ry_live  = (ΔGF − ΔCompressStore) / ΔGF         # runtime LOW_YIELD, live machine
```

`CompressStore` is the working set of the Memory Compression system process. It arrives free in
the existing §5.1 `NtQuerySystemInformation` sweep — no new syscall, no perf-counter dependency,
no additional cost at the moment the machine is already under pressure. The two samples in §1.2
are on-demand sweeps issued by the trim path, not reads of whatever the periodic sweep last
happened to store; the measurement window must be exactly the trim window.

`Ry_live` answers the only question the runtime check needs: of the memory we removed from the
group's working set, what fraction actually left RAM rather than being relabelled into the
compression store.

### 1.2 Runtime measurement protocol

One global trim lock, process-wide. A single trim is in flight at any time, ever.

```
trim_lock.acquire()
(gf0, cs0) = sample()
<trim pass>
wait 3 s
(gf1, cs1) = sample()
Ry_live = ((gf0 − gf1) − (cs1 − cs0)) / (gf0 − gf1)
trim_lock.release()
```

Serializing costs nothing: trims are already rate-limited to one per group per 20 s (§4.2), so
the lock is uncontended in practice. It removes RamJob-vs-RamJob contamination entirely.
OS-initiated trims of unrelated processes remain as residual noise, absorbed by the existing
rule that `LOW_YIELD` requires two consecutive bad samples.

### 1.3 Threshold calibration

The `Ry_live` cutoff is **not** guessed. M1 records both metrics on the same bench trims; the
cutoff is derived from the regression of `Ry_live` against `Ry_bench` at the 0.5 pass line. The
`0.35` currently in §4.2 becomes a placeholder explicitly marked *set at M1*.

Gate thresholds in §9.2 (pass ≥ 0.5, marginal 0.3–0.5, fail < 0.3) stand unchanged — they were
always `Ry_bench` numbers.

### 1.4 Pressure signal

`Available %` was the wrong signal: it counts the standby list, which is cache Windows hands
back for free. A machine can sit at 30% "available" while hard-faulting, or at 12% with no
pressure at all. v0.2 would arm and disarm at the wrong times in both directions.

`Available %` is demoted to a UI display number. Arm/disarm becomes:

```
ARM    = LowMemoryResourceNotification  signaled && hard_faults/s > 30, sustained 20 s
DISARM = HighMemoryResourceNotification signaled,                      sustained 60 s
```

`CreateMemoryResourceNotification` returns a waitable handle. Consequences:

- The 5 s pressure poll in §6.1 is **deleted**. Pressure becomes push-based, easing the idle
  CPU budget.
- The 15% / 25% / 2 GB / 3 GB tuning constants in §4.1 are **deleted**. Windows sets the
  threshold itself and it tracks the real machine.
- Hysteresis is preserved by the asymmetric low/high notification pair plus the dwell times.
- The hard-fault confirm prevents arming on a merely twitchy notification.

The §5.4 preflight note "Total RAM ≥ 32 GB → mostly dormant" stays true and strengthens: the
low-memory notification on a 32 GB machine almost never fires.

---

## 2. Group identity

*Amends §5.3. Adds to §5.2.*

### 2.1 Version-segment stripping

The install-root walk alone yields `%LOCALAPPDATA%\Discord\app-1.0.9036`. Every auto-update
mints a new group key and silently drops the user's cap. Discord, Slack, and Teams all behave
this way.

After the install-root walk, strip trailing version-shaped segments:

```
^(app-)?v?\d+(\.\d+)+$
^current$
```

Key = the lowercased, normalized remaining path. If the stripped root contains no launcher
executable, fall back one level up.

Signer subject remains tier 2 as in v0.2. It was rejected as tier 1 because it over-merges —
every Microsoft-signed application would collapse into a single group.

### 2.2 Shared runtime hosts

`msedgewebview2.exe` lives under one install root but hosts UI for many unrelated applications.
Naive grouping files every application's webview under "Microsoft Edge WebView2 Runtime".

Maintain a denylist of runtime-host image names:

```
msedgewebview2, java, javaw, python, pythonw, node, dotnet, wscript
```

A process whose image matches resolves its group by walking PPID to the first ancestor that
resolves to a non-runtime group; it joins that group. If no such ancestor exists, the process is
ungrouped and never displayed. The PPID walk happens once at process first sight and is cached
for the process lifetime alongside the existing image-path cache.

### 2.3 Self-exclusion

RamJob's own PID and every descendant are excluded from grouping, display, trimming, and job
assignment. This covers the Tauri WebView2 panel without depending on the runtime-host rule
above. Unstated in v0.2.

### 2.4 Membership churn during measurement

A browser spawns and reaps renderers constantly under pressure. A renderer exiting mid-window
reads as a wildly successful trim; a tab opening reads as a failed one.

Member PIDs are snapshotted at trim start. `ΔGF` sums only processes present in **both**
snapshots, validated by PID plus creation time. Processes that exited or spawned contribute
nothing to the yield math. The same intersection rule applies to the refault check in §4.2.

```
members_t0 = {pid: (ctime, wsp)}
members_t1 = sample()
common     = pids in both, matching ctime
ΔGF        = Σ_common (wsp_t0 − wsp_t1)
```

Only the private working set term appears. GF's unique-shared component (§3.1) is held constant
between refreshes by construction, so it cancels in any delta and must not be re-measured inside
a trim window — the dedup pass is far too expensive to run there.

### 2.5 M0 pass criterion

*Replaces the untestable "cleanly unify" in §10.*

Corpus: Brave, Chrome, VS Code, Discord, Slack, Spotify, Steam client, Teams, across three
machines. Every visible process hand-labeled once with its correct group.

```
pass = (correct_assignments / total >= 0.95) && (cross_app_merges == 0)
```

The merge condition is absolute: two different applications landing in one group fails the gate
regardless of the percentage.

---

## 3. Enforcement safety

*Amends §4.2.*

### 3.1 Foreground exclusion

v0.2's FSM would trim the window the user is actively typing in — a hard fault storm in the
foreground application is the most visible failure this product can produce.

At trim start, resolve the foreground PID via `GetForegroundWindow` → `GetWindowThreadProcessId`.
Exclude that PID and any group process owning a visible top-level window. Trim the remainder,
still descending by private working set.

```
fg_pid  = pid_of(GetForegroundWindow())
targets = group.procs − {fg_pid} − {p : p owns a visible top-level window}
```

Two calls, no polling. The browser case works well: background renderers are trimmed, the tab
being read is not. Skipping the whole group instead was rejected — it would permanently exempt
the largest consumer at exactly the moment pressure is highest.

### 3.2 Lowering a cap while the backstop is armed

`JobMemoryLimit` is never set below current group commit. On a cap decrease:

1. Do not raise the limit.
2. Run a soft trim pass.
3. Set `limit = max(target, current_commit × 1.05)`, where `target` is the §3.2 translation.
4. Repeat on subsequent passes until the limit reaches `target`.

The UI shows the cap as *applying* until it lands. Dragging a slider can never itself cause an
allocation failure.

The §3.2 rule that the backstop will not arm until `commit_ratio` has been sampled three times
is **retained**. The ratchet prevents an under-cap limit; the three-sample rule prevents an
absurdly high and therefore useless one. They cover different failures.

---

## 4. Supporting subsystems

*New sections in SPEC.md v0.3.*

### 4.1 Diagnostics

The product's normal state is doing nothing. Without a "why nothing happened" view, every
support report is unanswerable.

A 1024-entry in-memory ring buffer of state transitions and decisions: arm/disarm with the
signal values that caused them, trim attempts with skip reasons, every `Ry_live` sample,
`LOW_YIELD` and `THRASHING` marks, groups skipped as uncappable.

```
14:02:11 ARM lowmem=1 faults=47/s
14:02:31 brave TRIM 12 procs (fg skipped: 1)
14:02:34 brave Ry_live=0.22 gf-1.1GB cs+0.86GB
14:03:04 brave Ry_live=0.19 -> LOW_YIELD, stop
```

Lives in RAM. No disk writes, no rotation, no retention policy, no privacy surface. A panel
button copies the buffer plus the §5.4 preflight results to the clipboard. File logging exists
behind a config flag for development builds only.

The §7.4 honest-state messages source their "why" text from the same decision records.

### 4.2 History persistence

§7.5's floor needs the group's 24 h median GF; v0.2 never said where it lives.

Per group: 16 log-spaced GF buckets, ×1.5 apart, spanning 64 MB to 32 GB. Incremented on each
slow-poll sample, halved daily so the distribution tracks recent behavior. Median is read off
the cumulative sum.

```
on sample: b[bucket(gf)] += 1
daily:     b[i] *= 0.5
median   = smallest i where cumsum >= total/2
```

Roughly 40 bytes per group, stored in the config file. No separate store, no time series. Before
enough samples accumulate, the §7.5 floor falls back to the flat 300 MB.

An EMA was rejected: it is a mean, and one 60-tab browser session would drag a safety floor
upward for days.

### 4.3 Config lifecycle

- `version = 2` integer at the top of the file.
- An unknown version is backed up to `config.bak` and regenerated rather than parsed.
- Entries keyed by the §2.1 normalized root.
- Entries for groups not seen in 90 days are dropped on startup. `pinned = true` exempts an
  entry the user explicitly set.
- Atomic write via temp + rename, as v0.2 specified.

### 4.4 Panel ↔ core IPC

Tauri commands over the built-in bridge only. No localhost socket, no named pipe, no HTTP
listener.

The panel is a view. It sends `set_cap`, `set_flags`, `pause_all`, and `copy_diagnostics`, and
receives a group-state snapshot on a 1 s tick while open. The core never trusts a cap value from
the panel without re-applying the §7.5 floor on its own side.

---

## 5. Open questions from v0.2 — unchanged

These were not in scope for this pass and remain open:

1. Backstop defaults for Chromium-family apps (§4.2)
2. Runaway force-arm at 3× cap (§4.1)
3. `explorer.exe` advanced disclosure (§5.2)
4. VS Code sub-grouping (§5.3)
5. Battery behavior (§6.1)
6. Distribution format (§11)
7. Code signing / EV certificate (§11)

Question 2 is partly reshaped by §1.4: with a push-based OS pressure signal, "regardless of
system pressure" now means "regardless of the notification state", which is a cleaner condition
to reason about than a percentage threshold.
