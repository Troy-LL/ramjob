# RamJob — SPEC.md

**Version:** 0.3 (draft)
**Platform:** Windows 10 1809+ / Windows 11 (x64, ARM64)
**Status:** Pre-implementation. Sections marked `[OPEN]` need decisions.
**Changes from 0.2:** split yield into `Ry_bench` / `Ry_live` with a trim measurement
protocol (§2.3, §4.2, §9.2); pressure arms on OS memory-resource notifications + hard-fault
rate, not Available% (§4.1, §6.1); group identity gains version-segment stripping, runtime-host
denylist, and self-exclusion (§5.2–5.3); M0 pass criterion made measurable (§10); foreground
exclusion and JobMemoryLimit ratchet (§4.2); diagnostics ring, GF history buckets, config
lifecycle, and panel IPC (§8).

---

## 1. Product summary

A tray-resident utility that caps the memory usage of user-installed applications, grouped
automatically by application rather than by individual process. One knob per app. No process
lists, no PID hunting, no manual selection of the 14 `chrome.exe` children.

The mental model is the Opera GX RAM limiter, generalized: instead of one browser limiting
itself, one background service limits Brave, Spotify, VS Code, Discord, Slack, and anything
else the user points at.

**Non-goals (v1):** memory *cleaning* / "RAM booster" snake oil, per-tab control inside
browsers, GPU VRAM, network or CPU limiting, macOS/Linux, telemetry.

**Guiding principle:** RamJob must never make the machine slower than doing nothing. Free RAM
is not a virtue — Windows spends idle RAM on file cache, and reclaiming it for its own sake is
a net loss. Every enforcement action must be justified by actual memory pressure.

---

## 2. The three hard problems

Everything else in this spec is UI polish. These determine whether the product works at all.

### 2.1 What "limiting RAM" actually means on Windows

There is no single API that says "this app may use 4 GB." There are two mechanisms with
opposite tradeoffs:

| | Soft trim | Hard cap |
|---|---|---|
| API | `EmptyWorkingSet` / `SetProcessWorkingSetSizeEx` | Job Object + `JOBOBJECT_EXTENDED_LIMIT_INFORMATION.JobMemoryLimit` |
| Governs | Working set (physical residency) | **Commit**, not working set |
| Effect | Pages leave physical RAM. App keeps running, can fault them back. | Allocations beyond the cap **fail**. |
| Failure mode | Perf hit; thrash if the app re-touches immediately | App crashes, or handles OOM gracefully (most don't) |
| Reversible | Yes, instantly | Yes (limit raisable live) |
| Actually frees system memory | **Only partially — see §2.3** | Yes |

Opera GX's limiter is closer to the hard model, but it has the enormous advantage of running
*inside* the process it limits: it can discard tabs and drop caches cooperatively. We cannot
do that from outside. This asymmetry is the core risk of the project and must be reflected in
the UI (§7.4).

Note the row in bold: the soft path and the hard path govern **different quantities**. §3
resolves this.

### 2.2 Staying cheap while always running

A naive implementation enumerates every process every second and burns 2–4% CPU forever —
costing the user more than the RAM it saves. Budget and strategy in §6.

### 2.3 Memory compression may eat the entire benefit

Windows 10+ does not page trimmed memory straight to disk. It compresses it into an in-RAM
store owned by the Memory Compression system process. The observable consequence:

> Trim 2 GB from Brave. Task Manager shows Brave dropped 2 GB. System available memory rises
> by perhaps 700 MB. The other 1.3 GB is sitting compressed **in RAM**, just relabelled.

If the typical yield ratio is low, soft-only mode is theater — the UI would show large savings
that don't exist. This is the single biggest threat to the product's value proposition.

A single system-wide `Δ Available` cannot serve both an offline gate and a live policy check:
the former needs a quiesced machine; the latter runs on a busy one. Split into two metrics:

```
Ry_bench = Δ(Available MBytes) / Δ(GF)          # M1 gate only, quiesced machine
Ry_live  = (ΔGF − ΔCompressStore) / ΔGF         # runtime LOW_YIELD, live machine
```

`CompressStore` is the working set of the Memory Compression system process. It arrives free in
the existing §5.1 `NtQuerySystemInformation` sweep — no new syscall, no perf-counter dependency.

`Ry_live` answers the only question the runtime check needs: of the memory removed from the
group's working set, what fraction left RAM rather than being relabelled into the compression
store.

**Trim measurement protocol** (runtime). One global `trim_lock`, process-wide — a single trim
is in flight at any time. Samples are on-demand sweeps issued by the trim path, not stale
periodic reads; the window is exactly the trim window:

```
trim_lock.acquire()
(gf0, cs0) = sample()
<trim pass>
wait 3 s
(gf1, cs1) = sample()
Ry_live = ((gf0 − gf1) − (cs1 − cs0)) / (gf0 − gf1)
trim_lock.release()
```

Member PIDs are snapshotted at trim start. `ΔGF` sums **private working set only**, and only
for processes present in **both** snapshots, validated by PID + creation time. Exits and spawns
during the window contribute nothing. Unique-shared (§3.1) is held constant between refreshes
and must not be re-measured inside a trim window. The same intersection rule applies to the
refault check (§4.2).

Serializing costs nothing: trims are already one per group per 20 s (§4.2). OS-initiated trims
of unrelated processes remain residual noise, absorbed by requiring two consecutive bad
`Ry_live` samples for `LOW_YIELD`.

The `Ry_live` cutoff is **not** guessed. M1 records both metrics on the same bench trims; the
cutoff is derived from the regression of `Ry_live` against `Ry_bench` at the 0.5 pass line.
Until then, §4.2 uses a placeholder marked *set at M1*.

**`Ry_bench` is a go/no-go gate at M1, not a nice-to-have.** See §9.2 for the measurement
protocol and §9.3 for what we do if the number comes back bad.

---

## 3. Memory accounting — the canonical metric

Every number in the UI, every cap, and every policy threshold refers to one quantity. Defining
it precisely is not pedantry: naively summing Working Sets across a browser's 12 processes
double-counts every shared page, inflating "Brave: 4.2 GB" well past what Brave actually costs
the system.

### 3.1 Definition

**Group Footprint (GF)** = `Σ private working set of each process in the group`
                         + `unique shared working set attributed to the group, counted once`

- **Private working set** comes free: `SYSTEM_PROCESS_INFORMATION.WorkingSetPrivateSize` is
  already in the single `NtQuerySystemInformation` sweep (§5.1). Summing it across the group
  is correct by construction — private pages are by definition not shared, so no
  double-counting is possible.
- **Unique shared** (mostly DLLs and shared heaps, typically 100–400 MB per group) requires
  `QueryWorkingSetEx` per process and deduplication by page frame number. This is expensive
  (tens of ms for a large browser), so it runs **rarely**: once when a group is first tracked,
  then every 5 minutes, or on demand when the panel opens. Between refreshes the last value is
  held as a constant. It moves slowly; this is fine.

GF is what the slider caps, what the bars display, and what the policy FSM (§4.2) compares
against.

### 3.2 Reconciling the cap with the Job Object backstop

The backstop governs commit, so a GF cap cannot be handed to it directly. Translate using a
live per-group ratio:

```
commit_ratio = group commit charge / group GF      (sampled during PRESSURE, EMA-smoothed)
JobMemoryLimit = 1.15 × C × clamp(commit_ratio, 1.0, 2.0)
```

Group commit charge is `Σ PrivateUsage` from `PROCESS_MEMORY_COUNTERS_EX`. The clamp is a
safety rail: an unmeasured or wild ratio must never produce a job limit *below* the user's cap
(which would cause instant OOM) or absurdly above it (which would make the backstop useless).

Until a ratio has been sampled at least 3 times, the backstop **will not arm**. A group that
never reaches PRESSURE never gets a hard cap — correct, since it never needed one.

When lowering a cap while the backstop is already armed, never set `JobMemoryLimit` below
current group commit. Ratchet instead (§4.2).

### 3.3 What we do not display

No "RAM saved" counter. Given compression, pagefile, and shared pages, any single savings
number would be somewhere between misleading and fabricated. The UI shows current GF, the cap,
and system pressure. Nothing else. (Resolves open question 6 from v0.1.)

Available% from `GlobalMemoryStatusEx` is a **UI display number only** — it includes the standby
list and is not the arm/disarm signal (§4.1).

---

## 4. Enforcement model

### 4.1 Pressure gating (default ON)

Caps are **dormant until the system is actually short on memory**. On a 32 GB machine, holding
Brave to 6 GB while 18 GB sits unused makes the machine strictly slower for zero benefit.

Available% was the wrong signal: it counts the standby list, which is cache Windows hands back
for free. A machine can sit at 30% "available" while hard-faulting, or at 12% with no pressure
at all. Arm/disarm uses OS memory-resource notifications plus a hard-fault confirm:

```
ARM    = LowMemoryResourceNotification  signaled && hard_faults/s > 30, sustained 20 s
DISARM = HighMemoryResourceNotification signaled,                      sustained 60 s
```

`CreateMemoryResourceNotification` returns a waitable handle. Pressure is push-based — no
periodic Available% poll. Windows sets the threshold itself and it tracks the real machine.
Hysteresis comes from the asymmetric low/high notification pair plus the dwell times. The
hard-fault confirm prevents arming on a merely twitchy notification.

While DISARMED: no trims, no job limits armed, groups polled at the idle interval only. RamJob
is effectively asleep. This is the normal state on a well-provisioned machine and is what makes
the resource budget in §6 comfortably achievable. The §5.4 note that Total RAM ≥ 32 GB means
mostly dormant strengthens here: the low-memory notification almost never fires on such a
machine.

Per-app override in the `⚙` menu: **Always enforce** — ignore gating for this app. Intended for
the user who wants Brave held to 4 GB as a matter of principle. Off by default, with an inline
note that it may reduce performance when RAM is plentiful.

`[OPEN]` Should a group exceeding, say, 3× its cap force-arm regardless of notification state?
A runaway leak is worth catching before it becomes system-wide pressure. I lean yes, at 3×.

### 4.2 Policy FSM

Per group, cap `C`, evaluated only while **ARMED** (§4.1):

```
GF < 0.85C          → IDLE.      No action. Slow poll.
0.85C ≤ GF < 1.0C   → PRESSURE.  Fast poll. Sample commit_ratio. No action yet.
GF ≥ 1.0C           → TRIM.      Soft trim, largest private WS first, until GF < 0.9C
                                 or trim budget exhausted.
trim ineffective 3× → BACKSTOP.  Arm job memory limit per §3.2 (opt-in only).
in a 60 s window
```

**Trim mechanics**
- At trim start, resolve the foreground PID via `GetForegroundWindow` →
  `GetWindowThreadProcessId`. Exclude that PID and any group process owning a visible top-level
  window. Trim the remainder, still descending by private working set. Skipping the whole group
  was rejected — it would permanently exempt the largest consumer exactly when pressure is
  highest.
- Iterate the remaining processes descending by private working set.
- `SetProcessWorkingSetSizeEx(hProc, -1, -1, QUOTA_LIMITS_HARDWS_MIN_DISABLE)`, or
  `EmptyWorkingSet` for a full flush.
- Rate-limited: one trim pass per group per **20 s**, max 3 passes before escalating or giving
  up. This is the single most important anti-thrash rule. Global `trim_lock` (§2.3) serializes
  all trims process-wide.
- **Yield check.** After each trim, compute `Ry_live` (§2.3) under the measurement protocol.
  If `Ry_live` is below the cutoff (*placeholder 0.35, set at M1*) twice consecutively, the
  trim is moving memory into the compression store rather than freeing it — mark the group
  `LOW_YIELD`, stop trimming it, and surface it in the UI (§7.4). Escalate to backstop only if
  the user has opted in.
- **Refault check.** Using the same member intersection (PID + ctime) as §2.3: if a process's
  private working set returns to >90% of pre-trim within 5 s on two consecutive passes, mark
  `THRASHING`, stop, and surface it.

**Backstop mechanics**
- One Job Object per group, created lazily, limit computed per §3.2.
- `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` **must be off** — if RamJob crashes, the job handle
  closes and every limited app must survive.
- `JOB_OBJECT_LIMIT_BREAKAWAY_OK` off; children inherit the cap. This is what makes browser
  multi-process work.
- Already in another job: Win8+ nested jobs usually permit assignment. On failure, degrade to
  soft-only and say so.
- **Disarm on DISARM.** When system pressure clears, job limits are raised back to unlimited
  rather than left in place. A hard cap sitting armed on an idle system is a latent crash.
- **Lowering a cap while armed.** Never set `JobMemoryLimit` below current group commit.
  On a cap decrease: (1) do not raise the limit; (2) run a soft trim pass; (3) set
  `limit = max(target, current_commit × 1.05)`, where `target` is the §3.2 translation;
  (4) repeat on subsequent passes until the limit reaches `target`. The UI shows the cap as
  *applying* until it lands. Dragging a slider can never itself cause an allocation failure.
  The three-sample `commit_ratio` rule (§3.2) is retained — ratchet prevents an under-cap
  limit; three samples prevent an absurdly high and therefore useless one.

Backstop is **opt-in per app**, default off, with an explicit warning (§7.4).

`[OPEN]` Auto-enable the backstop for apps with known-good OOM handling? Browsers kill a single
renderer on OOM rather than dying. A small bundled profile list could flip the default for
Chromium-family apps only.

---

## 5. App discovery and grouping

### 5.1 Enumeration
`NtQuerySystemInformation(SystemProcessInformation)` returns every process with PID, PPID,
image name, session ID, and memory counters — including `WorkingSetPrivateSize` (§3.1) — in one
syscall, ~1–3 ms. Far cheaper than `EnumProcesses` plus per-PID opens. This is the workhorse.

Full image path via `QueryFullProcessImageName`, cached by PID+start-time so it resolves once
per process lifetime, never per poll.

### 5.2 Filtering — what the user sees
Show a process only if **all** of:
- Session ID ≠ 0 (excludes services)
- Image path not under `%WINDIR%` or `%ProgramFiles%\WindowsApps\Microsoft.*`
- Not on the critical denylist (`csrss`, `wininit`, `lsass`, `winlogon`, `services`, `smss`,
  `dwm`, `explorer`, `MsMpEng`, `SearchHost`, `ctfmon`, …)
- Not a Protected Process Light — we cannot touch these regardless
- **Not under a Steam/Epic/GOG game root.** Capping a game is almost always a mistake and an
  accidental one is a spectacular bug report. Detected via `steamapps\common`, Epic manifests,
  and the presence of common engine DLLs. Shown greyed with "games aren't capped" and an
  advanced override. (Resolves open question 4 from v0.1.)
- **Not RamJob itself.** RamJob's own PID and every descendant are excluded from grouping,
  display, trimming, and job assignment. This covers the Tauri WebView2 panel without relying
  on the runtime-host rule (§5.3).
- Group GF ≥ 50 MB

`[OPEN]` `explorer.exe` is denylisted. Some users will want it capped. Advanced disclosure, or
never?

### 5.3 Grouping
Group key resolution, in priority order:

1. **Install-root heuristic.** Walk up from the image path to the first directory that is a
   direct child of a known install root (`%ProgramFiles%`, `%LOCALAPPDATA%\Programs`,
   `%APPDATA%`, etc.). Everything under that root is one group. This is what correctly unifies
   `brave.exe` + 12 renderers + `brave_crashpad`.
   After the walk, strip trailing version-shaped segments matching
   `^(app-)?v?\d+(\.\d+)+$` or `^current$`. Key = the lowercased, normalized remaining path.
   If the stripped root contains no launcher executable, fall back one level up. Without this,
   Discord/Slack/Teams auto-updates mint a new group key and silently drop the user's cap.
2. **Authenticode signer subject.** Fallback for portable apps and dev builds. Rejected as
   tier 1 because it over-merges (every Microsoft-signed app would collapse into one group).
3. **Process tree root.** Fallback for unsigned portable apps: walk PPID to the session root.
4. **Image name.** Last resort.

**Shared runtime hosts.** Image names on the denylist
(`msedgewebview2`, `java`, `javaw`, `python`, `pythonw`, `node`, `dotnet`, `wscript`) do not
form their own group from the install-root walk. Resolve by walking PPID to the first ancestor
that resolves to a non-runtime group; join that group. If no such ancestor exists, the process
is ungrouped and never displayed. The PPID walk runs once at first sight and is cached for the
process lifetime alongside the image-path cache.

Display name from the version resource of the group's root executable ("Brave Browser", not
`brave.exe`). Icon extracted from the same binary, cached to disk as PNG on first sight.

**Group identity must be stable across reboots** — it is the config key. Use the normalized
(version-stripped) install-root path, or the signer subject hash if that was the resolution
tier. Never PID.

`[OPEN]` VS Code spawns extension hosts, terminals, and language servers. One group for v1 —
the premise is "don't make me pick individual processes" — but sub-groups may be wanted later.

### 5.4 Environment preflight (startup, once)
Detect conditions that change what RamJob can honestly promise:

| Condition | Consequence | Action |
|---|---|---|
| Pagefile disabled or < 1 GB | Soft trim has nowhere to spill beyond the compression store; yield will be poor | Warn once; recommend backstop-only or system-managed pagefile |
| Total RAM ≥ 32 GB | Low-memory notification almost never fires; pressure will rarely arm | Note in first-run that RamJob will mostly stay dormant — this is correct behaviour, not a bug |
| Pagefile on a small/aged SSD | Repeated trim cycles cause write amplification | Raise the trim rate limit from 20 s to 60 s |
| Not elevated | Cannot touch higher-integrity or other-user processes | Mark those groups as uncappable in the UI, no error spam |

---

## 6. Resource budget

Hard targets. Any build that misses these is a failed build.

| Metric | Target | Ceiling |
|---|---|---|
| Idle CPU (DISARMED, tray only) | < 0.1% | 0.3% |
| Active CPU (ARMED, 3 groups near cap) | < 0.5% | 1.5% |
| Idle working set (tray only) | < 12 MB | 25 MB |
| Working set with panel open | < 120 MB | 180 MB |
| Cold start to tray icon | < 400 ms | 1 s |

### 6.1 How we get there

**Pressure gating does most of the work.** While DISARMED (the common case on a healthy
machine) there is no per-group polling at all — only the slow sweep. Pressure itself is
push-based via `CreateMemoryResourceNotification` wait handles (§4.1); there is no periodic
Available% poll.

**Adaptive polling ladder**, per group, while ARMED:
- IDLE: 15 s · PRESSURE: 3 s · TRIM/BACKSTOP: 1 s, decaying back after 30 s of calm
- Full system sweep for discovery: 30 s armed, 120 s disarmed
- Panel open: 1 s full sweep for live bars; panel closed → immediately back down

**Event-driven process discovery.** Subscribe to the ETW `Microsoft-Windows-Kernel-Process`
provider for start/stop. Push-based, near-zero cost, and it removes the main reason to poll
fast. Requires an elevated or `PROFILE_USER` session; falls back to WMI
`__InstanceCreationEvent` on `Win32_Process`, then to sweep-only.

**Sleep the engine entirely** on screen lock, or when no group has a cap. Register for
`WM_POWERBROADCAST` and session notifications rather than polling for these.

**Coalesced wakeups.** One `SetWaitableTimerEx` with ~10% tolerable delay so the OS batches our
wakeups with others. Materially improves the battery answer.

**Never** `OpenProcess` in a hot loop. Handles for tracked groups are opened once and retained,
validated by PID+creation-time on reuse.

`[OPEN]` On battery: sleep the engine, or cap harder? Arguments both ways. Leaning: respect
pressure gating identically, but raise the trim rate limit to reduce disk writes.

---

## 7. UI

### 7.1 Tray
Single icon. Tooltip shows system memory pressure and whether RamJob is armed.
States: dormant (caps set, system healthy), armed (actively enforcing), warning (`LOW_YIELD`
or `THRASHING` group). Left click → panel. Right click → Pause all / Open / Settings / Quit.

### 7.2 Main panel
Popover anchored to the tray, ~420 × 600, not a window. Instant open, dismiss on focus loss.

```
┌─────────────────────────────────────────┐
│  RamJob            12.4 / 32 GB used    │
│  ▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░        │
│  ● Dormant — plenty of RAM free    ⓘ    │
├─────────────────────────────────────────┤
│  🅑  Brave Browser              4.2 GB  │
│      ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░  cap 6 GB     │
│      ●━━━━━━━━━━━━━○━━━━━━━━━━  [ ⚙ ]   │
│                                          │
│  🅢  Spotify                     680 MB  │
│      ▓▓▓▓▓▓▓░░░░░░░░░░░░░  cap 1 GB     │
│      ●━━━━○━━━━━━━━━━━━━━━━━━  [ ⚙ ]   │
│                                          │
│  🅥  Visual Studio Code          2.1 GB  │
│      unlimited                           │
│      ○┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈  [ ⚙ ]   │
├─────────────────────────────────────────┤
│  Show all apps (14) ▾      Pause all ⏸  │
└─────────────────────────────────────────┘
```

The status line under the system bar is load-bearing: it explains why a set cap isn't currently
doing anything. Without it, "Dormant" reads as broken. The `ⓘ` expands to: *"Your caps are set
but paused — capping apps when RAM is plentiful only makes things slower. RamJob will step in
automatically when memory gets tight."*

- Default sort: GF descending; capped apps pin to the top.
- Groups ≥ 50 MB by default; "Show all apps" expands.
- Bar fills against the cap when set, against total RAM when not.
- Slider snaps to 512 MB, 1, 1.5, 2, 3, 4, 6, 8, 12, 16 GB; fine control on shift-drag.
  Far-left = unlimited, the default for every app.
- Per-app `⚙`: hard backstop (opt-in), **always enforce** (§4.1), floor override, forget app.
- Diagnostics: a panel control copies the §8.1 ring buffer plus §5.4 preflight results to the
  clipboard.

### 7.3 First-run
No wizard. Show the panel with the top 5 consumers listed and a one-line explainer. On a
high-RAM machine, add the §5.4 note about expected dormancy. The user sets one slider and
closes it.

### 7.4 Honest states
The UI must not pretend this is free.

| State | Message |
|---|---|
| Cap below current usage | "Brave is using 4.2 GB. Capping at 2 GB will push memory out of RAM and may make it feel slower." |
| Enabling backstop | "If this app can't handle running out of memory, it may crash or lose unsaved work." |
| `LOW_YIELD` | "Capping this isn't freeing much — Windows is compressing the memory rather than releasing it. Raising the cap won't cost you much." |
| `THRASHING` | "Capping this isn't helping — it keeps reloading from disk. Consider raising the cap." + one-click raise |
| Uncappable (privilege) | "RamJob can't limit this app without administrator access." No repeated errors. |

Honest-state "why" text sources from the same decision records as §8.1.

### 7.5 Minimum floors
Never allow a cap below `max(300 MB, 0.25 × the group's observed 24 h median GF)`. Prevents
setting Discord to 128 MB and then reporting that Discord is broken. Median comes from the
§8.2 history buckets; before enough samples accumulate, fall back to the flat 300 MB.

---

## 8. Architecture

**Stack:** Rust (core + engine) + Tauri v2 (tray + WebView2 panel), `windows-rs` for Win32.

No GC means no periodic wakeups fighting the idle-CPU budget. `windows-rs` exposes
`NtQuerySystemInformation`, `QueryWorkingSetEx`, Job Objects, and ETW without P/Invoke
marshalling. Tauri lets the WebView2 host be **created on first panel open and destroyed on
close**, so steady state is the Rust binary alone (~8–12 MB). A .NET tray app idles at 60–80 MB
with GC timers; Electron would be self-parody for this product.

```
┌──────────────────────────────────────────────────┐
│ ramjob.exe (Rust, always resident)               │
│                                                  │
│  ┌────────────┐  ┌──────────────┐                │
│  │ Scanner    │→ │ Grouper      │                │
│  │ NtQSI+ETW  │  │ path/signer  │                │
│  └────────────┘  └──────┬───────┘                │
│                         ↓                         │
│  ┌────────────┐  ┌──────────────┐  ┌───────┐     │
│  │ Accountant │→ │ Group state  │→ │Config │     │
│  │ GF, ratio  │  │ store        │  │ TOML  │     │
│  └────────────┘  └──────┬───────┘  └───────┘     │
│                         ↓                         │
│  ┌────────────┐  ┌──────────────┐                │
│  │ Pressure   │→ │ Policy FSM   │                │
│  │ monitor    │  │ arm/disarm   │                │
│  └────────────┘  └──────┬───────┘                │
│                         ↓                         │
│  ┌──────────────────┐    ┌─────────────────┐     │
│  │ Enforcer         │    │ Tray + IPC      │     │
│  │ trim / joblimit  │    │                 │     │
│  └──────────────────┘    └────────┬────────┘     │
└───────────────────────────────────┼──────────────┘
                                    ↓ (lazy)
                          ┌─────────────────────┐
                          │ WebView2 panel      │
                          │ spawned on open,    │
                          │ killed on close     │
                          └─────────────────────┘
```

`Accountant` owns everything in §3: GF computation, the rare shared-page dedup pass,
commit_ratio EMA, and yield measurement (`Ry_live` / `Ry_bench`).

**Privileges:** non-elevated by default. `SeDebugPrivilege` is needed only for other-user or
higher-integrity processes; without it we say so rather than failing silently. An elevated
helper service is a v2 concern — shipping a service on day one doubles attack surface and
support burden.

**Autostart:** `HKCU\...\Run`. Not a service, not a scheduled task. User-visible, user-killable.

**Crash safety:** all job handles closed cleanly on exit; `KILL_ON_JOB_CLOSE` never set;
working-set trims are inherently non-persistent. Killing RamJob from Task Manager must leave
zero residue. **Fast user switching:** on session disconnect, release all job objects owned by
that session's groups — orphaned jobs across sessions are a known leak vector.

### 8.1 Diagnostics

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

### 8.2 History persistence

§7.5's floor needs the group's 24 h median GF. Per group: 16 log-spaced GF buckets, ×1.5 apart,
spanning 64 MB to 32 GB. Incremented on each slow-poll sample, halved daily so the distribution
tracks recent behavior. Median is read off the cumulative sum.

```
on sample: b[bucket(gf)] += 1
daily:     b[i] *= 0.5
median   = smallest i where cumsum >= total/2
```

Roughly 40 bytes per group, stored in the config file. No separate store, no time series. Before
enough samples accumulate, the §7.5 floor falls back to the flat 300 MB.

An EMA was rejected: it is a mean, and one 60-tab browser session would drag a safety floor
upward for days.

### 8.3 Config lifecycle

Path: `%APPDATA%\RamJob\config.toml`, human-editable.

- `version = 2` integer at the top of the file.
- An unknown version is backed up to `config.bak` and regenerated rather than parsed.
- Entries keyed by the §5.3 normalized (version-stripped) root.
- Entries for groups not seen in 90 days are dropped on startup. `pinned = true` exempts an
  entry the user explicitly set.
- Atomic write via temp + rename.
- History buckets (§8.2) live in the same file.

### 8.4 Panel ↔ core IPC

Tauri commands over the built-in bridge only. No localhost socket, no named pipe, no HTTP
listener.

The panel is a view. It sends `set_cap`, `set_flags`, `pause_all`, and `copy_diagnostics`, and
receives a group-state snapshot on a 1 s tick while open. The core never trusts a cap value from
the panel without re-applying the §7.5 floor on its own side.

---

## 9. Verification

Nothing ships on "it feels snappier." This category of software is where that claim is most
often wrong.

### 9.1 Harness
- **Synthetic hog**: a test process with scripted allocation and touch patterns — allocate-and-
  forget (trim-friendly), allocate-and-loop (thrash-prone), and sawtooth. Gives ground truth
  that a real browser cannot.
- **Real workloads**: scripted Brave session (30 tabs, mixed media), VS Code with a large repo
  and 3 language servers, Spotify with a long queue.
- **Instrumentation**, sampled at 1 Hz throughout: Available MBytes, Committed Bytes, Memory
  Compression process working set, pagefile write bytes/sec, per-group GF, RamJob's own CPU and
  working set. M1 also records `Ry_bench` and `Ry_live` on the same bench trims for cutoff
  calibration (§2.3).
- **Machine matrix**: 8 GB / 16 GB / 32 GB, HDD-pagefile and SSD-pagefile, Win10 and Win11.

### 9.2 The M1 compression gate
For each workload, on a quiesced machine, trim at a known GF and compute `Ry_bench` (§2.3)
over the 10 s following the trim.

- **Pass:** median `Ry_bench ≥ 0.5` across the real-workload corpus. Soft-first stays the
  primary path as specified.
- **Marginal:** `0.3 ≤ Ry_bench < 0.5`. Soft trim stays, but the backstop must be promoted to
  default-on for Chromium-family apps, and the UI language shifts from "limiting" to
  "encouraging".
- **Fail:** `Ry_bench < 0.3`. Soft trimming is theater. See §9.3.

Gate thresholds stand as `Ry_bench` numbers. The runtime `Ry_live` cutoff is derived at M1 from
the regression against `Ry_bench` at the 0.5 pass line (§2.3).

### 9.3 If the gate fails
The product pivots to **backstop-primary**: Job Object commit limits become the main mechanism,
soft trim is demoted to a pre-emptive nudge at 0.9C, and the risk warnings in §7.4 move from
opt-in fine print to the main flow. This is a materially different product with a higher crash
risk and a narrower safe app list, so it needs an explicit go/no-go conversation rather than a
silent slide. Do not skip this decision.

### 9.4 Continuous checks
Budget assertions from §6 run in CI against the synthetic hog: any commit that pushes idle CPU
above 0.3% or idle working set above 25 MB fails the build.

---

## 10. Milestones

| # | Deliverable | Proves |
|---|---|---|
| M0 | CLI: enumerate → group → print GF | Grouping heuristic meets the pass criterion below |
| **M1** | **Soft trim + harness + compression gate (§9.2)** | **The product's core value proposition is real** |
| M2 | Policy FSM + pressure gating | Enforcement is correct and doesn't thrash |
| M3 | Tray + panel + sliders | The UX premise |
| M4 | Job Object backstop with §3.2 translation, opt-in | Hard cap path |
| M5 | ETW discovery, adaptive polling, budget instrumentation in CI | §6 targets met |
| M6 | Config, autostart, preflight, first-run | Shippable |

**M0 pass criterion.** Corpus: Brave, Chrome, VS Code, Discord, Slack, Spotify, Steam client,
Teams, across three machines. Every visible process hand-labeled once with its correct group.

```
pass = (correct_assignments / total >= 0.95) && (cross_app_merges == 0)
```

The merge condition is absolute: two different applications landing in one group fails the gate
regardless of the percentage.

M0 and M1 are both gates, in that order. If M0 fails the criterion above, the product premise
fails. If the compression gate fails, the product changes shape. **Do not build UI before both
have passed** — everything downstream is wasted work otherwise.

---

## 11. Open questions

1. **Backstop defaults** (§4.2) — auto-enable for Chromium-family apps via a bundled profile
   list? Partly answered by the M1 gate outcome.
2. **Runaway override** (§4.1) — force-arm a group at 3× its cap regardless of notification
   state? Push-based pressure (§4.1) reshapes this: "regardless of system pressure" now means
   regardless of the memory-resource notification state, a cleaner condition than a percentage
   threshold.
3. **`explorer.exe`** (§5.2) — advanced disclosure, or never?
4. **VS Code sub-grouping** (§5.3) — one group, or split editor / ext-host / terminals?
5. **Battery** (§6.1) — sleep, or just slow the trim rate?
6. **Distribution** — MSIX, plain installer, or portable exe? Affects autostart and updates.
7. **Code signing** — unsigned binaries that open handles into other processes and assign job
   objects will be eaten alive by SmartScreen and AV heuristics. Budget for an EV cert?

*Resolved in 0.2:* games excluded by default (was 4); no "RAM saved" metric (was 6).
*Resolved in 0.3:* single `Ry` split; Available%-based arm/disarm replaced; group-identity and
trim-measurement gaps closed; supporting subsystems specified. See
`docs/superpowers/specs/2026-07-27-ramjob-spec-gaps-design.md`.
