# RamJob M3 — Tray panel UI (design)

**Date:** 2026-07-27  
**Status:** Approved (user lgtm on design file 2026-07-27)  
**Milestone:** M3 (SPEC §10 — Tray + panel + sliders)  
**Amends:** SPEC §7.1–7.2 (panel layout language); Arming semantics unchanged from SPEC §4.1 / M2  
**Lab:** `.superpowers/brainstorm/619-1785148579/` (visual companion)

---

## Problem

M2 proved Armed/Disarmed policy and per-group FSM via CLI/`config.toml`. M3 must prove the UX premise: a tray-resident instrument panel where the user sets caps the Opera GX way and understands dormancy vs arming vs warning — without building a second “dashboard” product.

## Goals

- Tray popover (~420 × 600) as the primary surface (SPEC §7.2 size).
- Light system chrome + instrument-cluster gauges (not dark GX chrome, not bar-only wireframe).
- System RAM history chart + live hero gauge; per-app gauges with **Opera-style adjustable limit markers**.
- Visual overall stop-loss (ceiling) with drag → release → history tick; **does not arm**.
- Clear state language: Armed ≠ Warning (blue vs red).
- Keep SPEC §7.3–7.5 (first-run, honest states, floors) and tray menu (§7.1).

## Non-goals (M3)

- Separate large dashboard window (deferred; tray is primary).
- Ceiling-driven Arming (rejected — visual annotation only).
- Job Object hard backstop UI beyond existing honest-state copy (M4 owns enforcement).
- ETW / budget CI (M5), autostart polish (M6).
- Replacing M2 policy/FSM semantics.

## Decisions (brainstorming)

| Topic | Decision |
|---|---|
| Chrome mood | Light system UI (not dark GX) |
| Visual language | Instrument-cluster gauges / speedometers |
| Warning color | Red |
| Armed color | Blue (distinct from Warning) |
| Primary surface | Tray popover ~420 × 600 |
| Top chart | System RAM over time (real curve, not stylized square wave) |
| Hero gauge | System RAM **now** (same metric as chart) |
| Overall ceiling | Amber dashed stop-loss on chart; **visual only** — does not arm |
| Ceiling vs OS arm | Arming remains SPEC §4.1 / M2 (OS pressure + runaway / always_enforce). Crossing ceiling does not arm |
| Ceiling interaction | Drag preview; **on release** commit new limit, stamp **dashed vertical** edit tick, continue **stepped** horizontal ceiling |
| Per-app caps | Each app gauge has its **own** Opera-style adjustable limit marker (drag) |
| App grid | ~3×2 gauges; scroll when more groups |
| SPEC §7.2 bars/sliders ASCII | Superseded by this instrument hub; snap sizes / floors / ⚙ / honest states still apply |

## Layout

```
┌─────────────────────────────────────────┐
│  RamJob     [blue Armed | Idle]    ⓘ    │
│  ┌──────────────────┐  ┌────────────┐   │
│  │ system RAM hist. │  │ hero gauge │   │
│  │ ~~~ curve ~~~    │  │  sys now   │   │
│  │ — — ceiling — —  │  │            │   │
│  │ | edit ticks     │  └────────────┘   │
│  └──────────────────┘                   │
├─────────────────────────────────────────┤
│  [app] [app]                            │
│  [app] [app]     ← gauges + limit marks │
│  [app] [app]                            │
│  (scroll)                               │
├─────────────────────────────────────────┤
│  Show all ▾              Pause all ⏸    │
└─────────────────────────────────────────┘
```

Popover anchored to tray; dismiss on focus loss. Instant open (SPEC budget).

## Interactions

### System ceiling

1. User drags amber ceiling (or handle) — live preview only; no new vertical tick while dragging.
2. On pointer-up / release — commit overall stop-loss value; append dashed **vertical** history marker at commit time; horizontal ceiling continues from that tick at the new level (stepped history).
3. Crossing the ceiling is a **visual** cue that usage went past the user’s stop-loss; it does **not** transition policy to Armed.

### Per-app limit marker

1. Each group dial shows current GF (needle/fill) and an adjustable cap marker (Opera-style).
2. Dragging the marker sets that group’s cap (same snap ladder as SPEC §7.2: 512 MB … 16 GB; far-left / far end = unlimited; shift-drag for fine control if feasible in WebView).
3. Markers are **per group**, independent of the system ceiling and of other apps.
4. Per-app `⚙` (hard backstop opt-in, always enforce, floor override, forget) remains available; primary cap edit is the dial marker.

### State chrome

| State | Presentation |
|---|---|
| Disarmed / dormant | Quiet Idle; status line explains dormancy (SPEC §7.2 ⓘ copy) |
| Armed | Blue pill / indicator — actively enforcing |
| Warning | Red — `LOW_YIELD` / `THRASHING` / honest-state warnings; never reuse Armed blue |

## Data / IPC (M3 shape)

- Panel is Tauri v2 WebView2 (SPEC §8); talks to Rust core for GF, caps, policy arm state, group list, honest-state reasons.
- Cap / overall-limit edits persist through existing config path (M2 `config.toml` lifecycle); UI is the editor, not a second source of truth.
- System RAM series + ceiling-edit timestamps feed the history chart (ring/history sufficient for panel lifetime; persistence detail in implementation plan).
- Diagnostics copy-to-clipboard control stays (SPEC §7.2 / §8.1).

## Relationship to Arming (explicit)

| Signal | Arms? |
|---|---|
| OS LowMemory + fault confirm (M2) | Yes |
| Runaway / always_enforce | Yes (existing rules) |
| Usage crosses visual ceiling | **No** — annotation / user stop-loss history only |
| Per-app over cap while Disarmed | No system arm by itself (FSM still Idle until Armed or always_enforce) |

## Out of scope polish (nice-later)

- Hybrid “Open dashboard” from popover.
- Double-click ceiling/marker → typed GB field (precision assist).
- Dark theme.

## Risks

| Risk | Mitigation |
|---|---|
| Small dials hard to drag precisely | Snap ladder + shift-drag; ⚙ remains escape hatch |
| Users think ceiling arms the product | ⓘ + status line; never color ceiling-cross as Armed |
| Chart + 6 gauges cramped in 420×600 | Scroll app grid; keep top row fixed |
| SPEC §7.2 ASCII vs gauges confuse implementers | This doc + SPEC §7.2 rewrite to instrument hub |

## Success (M3 UX)

- User can open tray → see system RAM history + now, set overall ceiling by drag-release, set per-app caps by dial markers, and read Armed (blue) vs Warning (red) correctly.
- Ceiling edits leave visible vertical ticks; Arming still matches M2 policy under simulate/live pressure.
- No second window required for the MVP panel.

---

## Next

1. ~~User reviews this file.~~ Approved.  
2. Implementation plan: `docs/superpowers/plans/2026-07-27-m3-tray-ui.md` → SDD on `milestone/m3-tray-ui`.  
3. SPEC §7.2 already folded to match (2026-07-27); §7.3–7.5 unchanged.
