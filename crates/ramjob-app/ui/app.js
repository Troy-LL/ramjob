// RamJob tray panel — shell wiring (Task 7), history chart (Task 8),
// hero + per-app gauges with Opera-style limit markers (Task 9).

const MIN_GF_BYTES = 50 * 1024 * 1024; // 50 MB "Show all apps" floor
const GB = 1024 * 1024 * 1024;

// Build some fake history: 30 samples over the last ~15 minutes, wobbling
// around 8-11 GB used out of 16 GB total, with one ceiling edit partway
// through (14 GB -> 12 GB) to exercise stepped-ceiling rendering.
function buildMockHistory() {
  const now = Date.now();
  const totalBytes = 16 * GB;
  const samples = [];
  for (let i = 0; i < 30; i++) {
    const t = now - (29 - i) * 30_000; // 30s apart
    const wobble = Math.sin(i / 4) * 1.2 * GB;
    const used = 9 * GB + wobble + (i > 20 ? 1.5 * GB : 0);
    samples.push({ unix_ms: t, used_bytes: Math.round(used), total_bytes: totalBytes });
  }
  const ceilingEdits = [{ unix_ms: now - 20 * 30_000, overall_limit_bytes: 14 * GB }];
  return { samples, ceilingEdits, totalBytes };
}

const mockHistory = buildMockHistory();

// Mock PanelSnapshot (crates/ramjob-core/src/panel.rs) for layout iteration
// when opening index.html directly in a browser (no window.__TAURI__).
const MOCK_SNAPSHOT = {
  system_arm: "Disarmed", // "Armed" | "Disarmed"
  pause_all: false,
  used_bytes: 10 * 1024 * 1024 * 1024,
  total_bytes: mockHistory.totalBytes,
  overall_limit_bytes: 12 * 1024 * 1024 * 1024,
  status_line: "Idle — caps set but paused until memory gets tight",
  warning: false,
  samples: mockHistory.samples,
  ceiling_edits: mockHistory.ceilingEdits,
  groups: [
    { key: "chrome", name: "Google Chrome", gf_bytes: 2.1 * 1024 * 1024 * 1024, cap_bytes: 4 * 1024 * 1024 * 1024, always_enforce: false, fsm_hint: "Idle" },
    { key: "slack", name: "Slack", gf_bytes: 900 * 1024 * 1024, cap_bytes: 0, always_enforce: false, fsm_hint: "Idle" },
    { key: "vscode", name: "Visual Studio Code", gf_bytes: 1.3 * 1024 * 1024 * 1024, cap_bytes: 0, always_enforce: false, fsm_hint: "Pressure" },
    { key: "docker", name: "Docker Desktop", gf_bytes: 3.4 * 1024 * 1024 * 1024, cap_bytes: 6 * 1024 * 1024 * 1024, always_enforce: true, fsm_hint: "Idle" },
    { key: "spotify", name: "Spotify", gf_bytes: 30 * 1024 * 1024, cap_bytes: 0, always_enforce: false, fsm_hint: "Idle" },
    { key: "figma", name: "Figma", gf_bytes: 420 * 1024 * 1024, cap_bytes: 0, always_enforce: false, fsm_hint: "Idle" },
  ],
};

const isTauri = () => typeof window !== "undefined" && !!window.__TAURI__;

async function ipc(cmd, args) {
  if (!isTauri()) return null;
  const { invoke } = window.__TAURI__.core ?? window.__TAURI__.tauri;
  return args === undefined ? invoke(cmd) : invoke(cmd, args);
}

async function fetchSnapshot() {
  const live = await ipc("get_snapshot");
  return live ?? MOCK_SNAPSHOT;
}

async function callPauseAll(pause) {
  const live = await ipc("pause_all", { pause });
  if (live) return live;
  MOCK_SNAPSHOT.pause_all = pause;
  return MOCK_SNAPSHOT;
}

// Mirrors crates/ramjob-core/src/cap_math.rs snap_cap_bytes — server is the
// source of truth on release, this is just for a directionally-correct
// live drag preview.
const CAP_SNAP_BYTES = [0.5, 1, 1.5, 2, 3, 4, 6, 8, 12, 16].map((g) => g * GB);
function snapCapBytesPreview(raw, shiftFine) {
  if (raw <= 0) return 0;
  if (shiftFine) {
    const unit = 64 * 1024 * 1024;
    return Math.max(unit, Math.round(raw / unit) * unit);
  }
  const threshold = CAP_SNAP_BYTES[0] / 2;
  if (raw < threshold) return 0;
  return CAP_SNAP_BYTES.reduce((best, snap) =>
    Math.abs(raw - snap) < Math.abs(raw - best) ? snap : best
  );
}

// Mirrors crates/ramjob-core/src/cap_math.rs snap_ceiling_bytes — the system
// ceiling is a separate, unbounded concept from the per-app cap ladder above
// (no 16GB top), so it gets its own preview snap instead of reusing
// snapCapBytesPreview.
function snapCeilingBytesPreview(raw, shiftFine) {
  if (raw <= 0) return 0;
  const unit = shiftFine ? 64 * 1024 * 1024 : GB;
  return Math.max(unit, Math.round(raw / unit) * unit);
}

async function callSetOverallLimit(limitBytes, shiftFine) {
  const live = await ipc("set_overall_limit", { limitBytes, shiftFine });
  if (live) return live;
  const snapped = snapCeilingBytesPreview(limitBytes, shiftFine);
  MOCK_SNAPSHOT.overall_limit_bytes = snapped;
  MOCK_SNAPSHOT.ceiling_edits.push({ unix_ms: Date.now(), overall_limit_bytes: snapped });
  return MOCK_SNAPSHOT;
}

async function callSetCap(key, capBytes, shiftFine) {
  const live = await ipc("set_cap", { key, capBytes, shiftFine });
  if (live) return live;
  const snapped = snapCapBytesPreview(capBytes, shiftFine);
  const g = MOCK_SNAPSHOT.groups.find((g) => g.key === key);
  if (g) g.cap_bytes = snapped;
  return MOCK_SNAPSHOT;
}

async function callCopyDiagnostics() {
  await ipc("copy_diagnostics");
}

async function callSetFlags(key, alwaysEnforce) {
  const live = await ipc("set_flags", { key, alwaysEnforce });
  if (live) return live;
  const g = MOCK_SNAPSHOT.groups.find((g) => g.key === key);
  if (g) g.always_enforce = alwaysEnforce;
  return MOCK_SNAPSHOT;
}

function formatBytes(b) {
  const gb = b / (1024 * 1024 * 1024);
  return gb >= 1 ? `${gb.toFixed(1)} GB` : `${Math.round(b / (1024 * 1024))} MB`;
}

function renderPill(snapshot) {
  const pill = document.getElementById("state-pill");
  const dot = document.getElementById("status-dot");
  pill.classList.remove("pill-idle", "pill-armed", "pill-warning");
  dot.classList.remove("armed", "warning");

  if (snapshot.warning) {
    pill.textContent = "Warning";
    pill.classList.add("pill-warning");
    dot.classList.add("warning");
  } else if (snapshot.system_arm === "Armed") {
    pill.textContent = "Armed";
    pill.classList.add("pill-armed");
    dot.classList.add("armed");
  } else {
    pill.textContent = "Idle";
    pill.classList.add("pill-idle");
  }
}

// Same ladder as cap_math::CAP_SNAP_BYTES — the shared angular scale for every
// per-app dial, so an app's marker and fill line up regardless of its cap.
const DIAL_LADDER_MAX = 16 * GB;

// Per-key live drag preview. Stashes frozen dialMax so unlimited→cap drags
// don't flip the angular scale mid-gesture (which pinned the marker to 1.0).
const capDragPreview = {}; // key -> { bytes, dialMax }

function honestMessage(fsmHint, group) {
  if (group && group.cap_bytes > 0 && group.gf_bytes > group.cap_bytes) {
    return `${group.name} is using ${formatBytes(group.gf_bytes)}. Capping at ${formatBytes(group.cap_bytes)} will push memory out of RAM and may make it feel slower.`;
  }
  if (fsmHint === "LowYield") {
    return "Capping this isn't freeing much — Windows is compressing the memory rather than releasing it. Raising the cap won't cost you much.";
  }
  if (fsmHint === "Thrashing") {
    return "Capping this isn't helping — it keeps reloading from disk. Consider raising the cap.";
  }
  return null;
}

// Semicircle arc gauge (Opera-style): 180deg at the left (value=0) sweeping
// through the top down to 0deg at the right (value=max). `fillFrac` draws the
// colored arc; `markerFrac` (optional) draws a draggable dot at that fraction.
function buildArcGauge({ width, height, fillFrac, fillColor, markerFrac, markerColor, onDrag, onCommit }) {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("width", "100%");
  svg.setAttribute("height", "100%");
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  svg.style.display = "block";
  svg.style.touchAction = "none";

  const cx = width / 2;
  const cy = height - 6;
  const r = Math.min(width / 2 - 6, height - 12);

  const pointAt = (frac) => {
    const angleDeg = 180 - Math.max(0, Math.min(1, frac)) * 180;
    const rad = (angleDeg * Math.PI) / 180;
    return { x: cx + r * Math.cos(rad), y: cy - r * Math.sin(rad) };
  };
  const arcPath = (fromFrac, toFrac) => {
    const p0 = pointAt(fromFrac);
    const p1 = pointAt(toFrac);
    const largeArc = toFrac - fromFrac > 0.5 ? 1 : 0;
    return `M${p0.x.toFixed(1)},${p0.y.toFixed(1)} A${r.toFixed(1)},${r.toFixed(1)} 0 ${largeArc} 1 ${p1.x.toFixed(1)},${p1.y.toFixed(1)}`;
  };

  const track = document.createElementNS(SVG_NS, "path");
  track.setAttribute("d", arcPath(0, 1));
  track.setAttribute("fill", "none");
  track.setAttribute("stroke", "#e4e6e9");
  track.setAttribute("stroke-width", "6");
  track.setAttribute("stroke-linecap", "round");
  svg.appendChild(track);

  if (fillFrac > 0) {
    const fill = document.createElementNS(SVG_NS, "path");
    fill.setAttribute("d", arcPath(0, Math.min(1, fillFrac)));
    fill.setAttribute("fill", "none");
    fill.setAttribute("stroke", fillColor);
    fill.setAttribute("stroke-width", "6");
    fill.setAttribute("stroke-linecap", "round");
    svg.appendChild(fill);
  }

  if (markerFrac != null) {
    const p = pointAt(markerFrac);
    const marker = document.createElementNS(SVG_NS, "circle");
    marker.setAttribute("cx", p.x.toFixed(1));
    marker.setAttribute("cy", p.y.toFixed(1));
    marker.setAttribute("r", "5.5");
    marker.setAttribute("fill", markerColor);
    marker.setAttribute("stroke", "#fff");
    marker.setAttribute("stroke-width", "1.5");
    if (onDrag && onCommit) {
      marker.style.cursor = "ew-resize";

      const fracFromPointer = (clientX, clientY) => {
        const box = svg.getBoundingClientRect();
        const scaleX = width / box.width;
        const scaleY = height / box.height;
        const x = (clientX - box.left) * scaleX;
        const y = (clientY - box.top) * scaleY;
        const angle = Math.atan2(cy - y, x - cx); // 0..PI across the top
        const clamped = Math.max(0, Math.min(Math.PI, angle));
        return 1 - clamped / Math.PI;
      };

      marker.addEventListener("pointerdown", (ev) => {
        ev.preventDefault();
        const pointerId = ev.pointerId;
        // Document listeners survive preview DOM updates; also listen for
        // pointercancel so cancelled gestures don't leak handlers.
        const onMove = (moveEv) => {
          if (moveEv.pointerId !== pointerId) return;
          const frac = fracFromPointer(moveEv.clientX, moveEv.clientY);
          const p = pointAt(frac);
          marker.setAttribute("cx", p.x.toFixed(1));
          marker.setAttribute("cy", p.y.toFixed(1));
          onDrag(frac);
        };
        const teardown = async (upEv) => {
          if (upEv.pointerId !== pointerId) return;
          document.removeEventListener("pointermove", onMove);
          document.removeEventListener("pointerup", teardown);
          document.removeEventListener("pointercancel", teardown);
          if (upEv.type === "pointercancel") {
            onDrag(markerFrac);
            return;
          }
          await onCommit(fracFromPointer(upEv.clientX, upEv.clientY), upEv.shiftKey);
        };
        document.addEventListener("pointermove", onMove);
        document.addEventListener("pointerup", teardown);
        document.addEventListener("pointercancel", teardown);
      });
    }
    svg.appendChild(marker);
  }

  return svg;
}

function renderHeroGauge(snapshot) {
  const container = document.getElementById("hero-gauge");
  container.innerHTML = "";
  const rect = container.getBoundingClientRect();
  const width = rect.width || 120;
  const height = rect.height || 130;

  const total = snapshot.total_bytes || 1;
  const fillFrac = snapshot.used_bytes / total;
  const svg = buildArcGauge({ width, height, fillFrac, fillColor: "#2f6fed" });
  container.appendChild(svg);

  const label = document.createElement("div");
  label.className = "hero-gauge-label";
  label.textContent = `${formatBytes(snapshot.used_bytes)} / ${formatBytes(total)}`;
  container.appendChild(label);
}

// Default sort (SPEC §7.2): capped apps pin to the top; GF descending within
// each bucket (capped, then uncapped).
function sortGroups(groups) {
  return [...groups].sort((a, b) => {
    const aCapped = a.cap_bytes > 0;
    const bCapped = b.cap_bytes > 0;
    if (aCapped !== bCapped) return aCapped ? -1 : 1;
    return b.gf_bytes - a.gf_bytes;
  });
}

function renderAppGrid(snapshot, showAll, onCommitCap, onToggleFlag) {
  const grid = document.getElementById("app-grid");
  grid.innerHTML = "";

  const groups = sortGroups(snapshot.groups);
  const aboveFloor = groups.filter((g) => g.gf_bytes >= MIN_GF_BYTES);
  const anyCapped = groups.some((g) => g.cap_bytes > 0);
  // SPEC §7.3 first-run: top 5. Default: ≥50MB. Show all: every IPC group.
  let visible;
  if (showAll) {
    visible = groups;
  } else if (!anyCapped) {
    visible = aboveFloor.slice(0, 5);
  } else {
    visible = aboveFloor;
  }
  document.getElementById("hidden-count").textContent = String(
    Math.max(0, groups.length - visible.length)
  );
  const toggle = document.getElementById("show-all-toggle");
  if (toggle) {
    const hiddenN = Math.max(0, groups.length - visible.length);
    toggle.textContent = showAll ? "Show fewer apps ▴" : `Show all apps (${hiddenN}) ▾`;
  }

  for (const g of visible) {
    const card = document.createElement("div");
    card.className = "app-card";

    const header = document.createElement("div");
    header.className = "app-card-header";
    const nameEl = document.createElement("div");
    nameEl.className = "app-name";
    nameEl.textContent = g.name;
    const gearBtn = document.createElement("button");
    gearBtn.className = "gear-btn";
    gearBtn.textContent = "⚙";
    gearBtn.title = "App options";
    header.append(nameEl, gearBtn);
    card.appendChild(header);

    const gaugeWrap = document.createElement("div");
    gaugeWrap.className = "app-gauge";
    const rect = { width: 176, height: 62 }; // fits the ~193px card minus padding
    const preview = capDragPreview[g.key];
    const committedCap = g.cap_bytes;
    // Freeze dialMax for the whole gesture once a preview exists.
    const dialMax =
      preview?.dialMax ??
      (committedCap > 0
        ? Math.max(committedCap, g.gf_bytes, CAP_SNAP_BYTES[0])
        : Math.max(snapshot.total_bytes || DIAL_LADDER_MAX, g.gf_bytes, CAP_SNAP_BYTES[0]));
    const capBytes = preview != null ? preview.bytes : committedCap;
    const fillFrac = Math.min(1, g.gf_bytes / dialMax);
    const markerFrac = Math.min(1, (capBytes > 0 ? capBytes : dialMax) / dialMax);

    const meta = document.createElement("div");
    meta.className = "app-meta";
    meta.textContent = `${formatBytes(g.gf_bytes)}${g.cap_bytes ? " / cap " + formatBytes(g.cap_bytes) : " / unlimited"}`;

    const gaugeSvg = buildArcGauge({
      width: rect.width,
      height: rect.height,
      fillFrac,
      fillColor: "#2f6fed",
      markerFrac,
      markerColor: "#c8860d",
      onDrag: (frac) => {
        const bytes = Math.round(frac * dialMax);
        capDragPreview[g.key] = { bytes, dialMax };
        meta.textContent = `${formatBytes(g.gf_bytes)} / cap ${formatBytes(bytes || 0)}`;
      },
      onCommit: async (frac, shiftFine) => {
        delete capDragPreview[g.key];
        await onCommitCap(g.key, Math.round(frac * dialMax), shiftFine);
      },
    });
    gaugeWrap.appendChild(gaugeSvg);
    card.appendChild(gaugeWrap);
    card.appendChild(meta);

    const honestText = honestMessage(g.fsm_hint, g);
    if (honestText) {
      const warn = document.createElement("div");
      warn.className = "app-honest";
      warn.textContent = honestText;
      card.appendChild(warn);
    }

    const popover = document.createElement("div");
    popover.className = "gear-popover hidden";
    const label = document.createElement("label");
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.checked = g.always_enforce;
    checkbox.addEventListener("change", async () => {
      await onToggleFlag(g.key, checkbox.checked);
    });
    label.append(checkbox, document.createTextNode(" Always enforce (hard backstop)"));
    popover.appendChild(label);
    card.appendChild(popover);

    gearBtn.addEventListener("click", () => {
      popover.classList.toggle("hidden");
    });

    grid.appendChild(card);
  }
}

// SPEC §7.3: no wizard — just a one-line explainer, shown only while the
// panel is in its true first-run state (nothing capped yet, few apps visible).
function renderFirstRunHint(snapshot, _showAll) {
  const hint = document.getElementById("first-run-hint");
  const anyCapped = snapshot.groups.some((g) => g.cap_bytes > 0);
  const totalGb = (snapshot.total_bytes || 0) / GB;
  hint.textContent =
    totalGb >= 32
      ? "Set a cap on any app below to get started — on a high-RAM machine RamJob stays dormant until memory gets tight."
      : "Set a cap on any app below to get started — RamJob only steps in when memory gets tight.";
  // SPEC §7.3: one-line explainer while nothing is capped yet.
  hint.classList.toggle("hidden", anyCapped);
}

function renderStatusLine(snapshot) {
  document.getElementById("status-text").textContent = snapshot.status_line;
}

function renderPauseButton(snapshot) {
  const btn = document.getElementById("pause-all-btn");
  btn.classList.toggle("active", snapshot.pause_all);
  btn.textContent = snapshot.pause_all ? "Resume ▶" : "Pause all ⏸";
}

const SVG_NS = "http://www.w3.org/2000/svg";
const CHART_PAD = 6;
// Local drag preview state — never touches IPC until pointer-up commits it.
const dragState = { active: false, previewBytes: 0 };

// Reconstruct the ceiling step function as a list of {startMs, endMs, bytes}
// segments: each ceiling_edits[i] holds from its own timestamp until the
// next edit (or "now"), and the level *before* the first edit is unknown —
// we just start the first segment at the first sample's time.
function ceilingSegments(samples, ceilingEdits, currentLimitBytes, nowMs) {
  const edits = [...ceilingEdits].sort((a, b) => a.unix_ms - b.unix_ms);
  const startMs = samples.length ? samples[0].unix_ms : nowMs;
  const segments = [];

  if (edits.length === 0) {
    segments.push({ startMs, endMs: nowMs, bytes: currentLimitBytes });
    return segments;
  }

  // Flat at the level of the first edit, from chart start up to that edit.
  if (edits[0].unix_ms > startMs) {
    segments.push({ startMs, endMs: edits[0].unix_ms, bytes: edits[0].overall_limit_bytes });
  }
  for (let i = 0; i < edits.length; i++) {
    const segStart = edits[i].unix_ms;
    const segEnd = i + 1 < edits.length ? edits[i + 1].unix_ms : nowMs;
    segments.push({ startMs: segStart, endMs: segEnd, bytes: edits[i].overall_limit_bytes });
  }
  // Current/final level continues from the last edit through now (already
  // covered above since edits[last].overall_limit_bytes should equal
  // currentLimitBytes in a consistent snapshot; if not, trust currentLimitBytes).
  segments[segments.length - 1].bytes = currentLimitBytes;
  return segments;
}

// Renders the system RAM history chart (used-bytes curve + stepped amber
// dashed ceiling with vertical edit ticks + a draggable handle) into
// #history-chart. Returns nothing; wires its own pointer drag state.
function renderHistoryChart(snapshot, onCommitLimit) {
  const container = document.getElementById("history-chart");
  const rect = container.getBoundingClientRect();
  const width = rect.width || 260;
  const height = rect.height || 130;

  container.innerHTML = "";
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("width", "100%");
  svg.setAttribute("height", "100%");
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  svg.style.display = "block";
  svg.style.touchAction = "none";

  const samples = snapshot.samples ?? [];
  const total = snapshot.total_bytes || 1;
  const nowMs = Date.now();
  const t0 = samples.length ? samples[0].unix_ms : nowMs - 1;
  const t1 = samples.length ? samples[samples.length - 1].unix_ms : nowMs;
  const tSpan = Math.max(1, t1 - t0);

  const xOf = (ms) => CHART_PAD + ((ms - t0) / tSpan) * (width - 2 * CHART_PAD);
  const yOf = (bytes) => height - CHART_PAD - (bytes / total) * (height - 2 * CHART_PAD);

  // Used-bytes curve.
  if (samples.length > 1) {
    const d = samples
      .map((s, i) => `${i === 0 ? "M" : "L"}${xOf(s.unix_ms).toFixed(1)},${yOf(s.used_bytes).toFixed(1)}`)
      .join(" ");
    const path = document.createElementNS(SVG_NS, "path");
    path.setAttribute("d", d);
    path.setAttribute("fill", "none");
    path.setAttribute("stroke", "#2f6fed");
    path.setAttribute("stroke-width", "1.5");
    svg.appendChild(path);
  }

  // Preview level while dragging overrides the "current" level for display.
  const displayLimit = dragState.active ? dragState.previewBytes : snapshot.overall_limit_bytes;
  const segments = ceilingSegments(samples, snapshot.ceiling_edits ?? [], displayLimit, nowMs);

  for (const seg of segments) {
    const line = document.createElementNS(SVG_NS, "line");
    line.setAttribute("x1", xOf(seg.startMs).toFixed(1));
    line.setAttribute("x2", xOf(seg.endMs).toFixed(1));
    line.setAttribute("y1", yOf(seg.bytes).toFixed(1));
    line.setAttribute("y2", yOf(seg.bytes).toFixed(1));
    line.setAttribute("stroke", "#c8860d");
    line.setAttribute("stroke-width", "1.5");
    line.setAttribute("stroke-dasharray", "4 3");
    svg.appendChild(line);
  }

  // Vertical dashed ticks at each committed edit — never while dragging.
  for (const edit of snapshot.ceiling_edits ?? []) {
    const x = xOf(edit.unix_ms);
    const tick = document.createElementNS(SVG_NS, "line");
    tick.setAttribute("x1", x.toFixed(1));
    tick.setAttribute("x2", x.toFixed(1));
    tick.setAttribute("y1", String(CHART_PAD));
    tick.setAttribute("y2", String(height - CHART_PAD));
    tick.setAttribute("stroke", "#c8860d");
    tick.setAttribute("stroke-width", "1");
    tick.setAttribute("stroke-dasharray", "2 2");
    tick.setAttribute("opacity", "0.6");
    svg.appendChild(tick);
  }

  // Draggable handle at the right edge, on the current/preview ceiling level.
  const handleY = yOf(displayLimit);
  const handleX = width - CHART_PAD;
  const handle = document.createElementNS(SVG_NS, "circle");
  handle.setAttribute("cx", handleX.toFixed(1));
  handle.setAttribute("cy", handleY.toFixed(1));
  handle.setAttribute("r", "5");
  handle.setAttribute("fill", "#c8860d");
  handle.setAttribute("stroke", "#fff");
  handle.setAttribute("stroke-width", "1.5");
  handle.style.cursor = "ns-resize";
  svg.appendChild(handle);

  // Live preview of the ceiling height while dragging (no full chart rebuild —
  // rebuilding would detach this SVG and break bytesFromY / pointer handlers).
  const previewLine = document.createElementNS(SVG_NS, "line");
  previewLine.setAttribute("x1", String(CHART_PAD));
  previewLine.setAttribute("x2", String(width - CHART_PAD));
  previewLine.setAttribute("y1", handleY.toFixed(1));
  previewLine.setAttribute("y2", handleY.toFixed(1));
  previewLine.setAttribute("stroke", "#c8860d");
  previewLine.setAttribute("stroke-width", "1.5");
  previewLine.setAttribute("stroke-dasharray", "4 3");
  previewLine.setAttribute("opacity", "0");
  svg.appendChild(previewLine);
  svg.appendChild(handle);

  const bytesFromY = (clientY) => {
    const box = svg.getBoundingClientRect();
    const scale = height / box.height;
    const y = (clientY - box.top) * scale;
    const clampedY = Math.min(height - CHART_PAD, Math.max(CHART_PAD, y));
    const frac = 1 - (clampedY - CHART_PAD) / (height - 2 * CHART_PAD);
    return Math.max(0, Math.round(frac * total));
  };

  handle.addEventListener("pointerdown", (ev) => {
    ev.preventDefault();
    const pointerId = ev.pointerId;
    dragState.active = true;
    dragState.previewBytes = snapshot.overall_limit_bytes;
    previewLine.setAttribute("opacity", "1");

    const onMove = (moveEv) => {
      if (moveEv.pointerId !== pointerId) return;
      dragState.previewBytes = bytesFromY(moveEv.clientY);
      const y = yOf(dragState.previewBytes);
      handle.setAttribute("cy", y.toFixed(1));
      previewLine.setAttribute("y1", y.toFixed(1));
      previewLine.setAttribute("y2", y.toFixed(1));
    };
    const teardown = async (upEv) => {
      if (upEv.pointerId !== pointerId) return;
      document.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerup", teardown);
      document.removeEventListener("pointercancel", teardown);
      const finalBytes = dragState.previewBytes;
      const shiftFine = upEv.shiftKey;
      dragState.active = false;
      previewLine.setAttribute("opacity", "0");
      if (upEv.type === "pointercancel") {
        renderHistoryChart(snapshot, onCommitLimit);
        return;
      }
      await onCommitLimit(finalBytes, shiftFine);
    };
    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup", teardown);
    document.addEventListener("pointercancel", teardown);
  });

  container.appendChild(svg);
}

async function main() {
  let snapshot = await fetchSnapshot();
  let showAll = false;
  let lastFingerprint = JSON.stringify(snapshot);

  const applySnapshot = (next) => {
    snapshot = next;
    lastFingerprint = JSON.stringify(next);
  };

  const render = () => {
    renderPill(snapshot);
    renderStatusLine(snapshot);
    renderFirstRunHint(snapshot, showAll);
    renderHeroGauge(snapshot);
    renderAppGrid(
      snapshot,
      showAll,
      async (key, capBytes, shiftFine) => {
        applySnapshot(await callSetCap(key, capBytes, shiftFine));
        render();
      },
      async (key, alwaysEnforce) => {
        applySnapshot(await callSetFlags(key, alwaysEnforce));
        render();
      }
    );
    renderPauseButton(snapshot);
    renderHistoryChart(snapshot, async (limitBytes, shiftFine) => {
      applySnapshot(await callSetOverallLimit(limitBytes, shiftFine));
      render();
    });
  };
  render();

  document.getElementById("info-btn").addEventListener("click", () => {
    const popover = document.getElementById("info-popover");
    const expanded = !popover.classList.contains("hidden");
    popover.classList.toggle("hidden", expanded);
    document.getElementById("info-btn").setAttribute("aria-expanded", String(!expanded));
  });

  document.getElementById("show-all-toggle").addEventListener("click", () => {
    showAll = !showAll;
    render();
  });

  document.getElementById("copy-diagnostics-btn").addEventListener("click", async () => {
    const btn = document.getElementById("copy-diagnostics-btn");
    await callCopyDiagnostics();
    const original = btn.textContent;
    btn.textContent = "Copied ✓";
    btn.classList.add("copied");
    setTimeout(() => {
      btn.textContent = original;
      btn.classList.remove("copied");
    }, 1500);
  });

  document.getElementById("pause-all-btn").addEventListener("click", async () => {
    applySnapshot(await callPauseAll(!snapshot.pause_all));
    renderPauseButton(snapshot);
    renderStatusLine(snapshot);
  });

  // SPEC §8.4: refresh the snapshot on a 1s tick while the panel is open, so
  // it doesn't stay frozen at startup values until a user interaction forces
  // a re-render. Skipped while a drag preview is live (ceiling drag or a
  // per-app cap drag) so a poll-driven re-render can't fight/reset it.
  setInterval(async () => {
    if (dragState.active || Object.keys(capDragPreview).length > 0) return;
    const next = await fetchSnapshot();
    const fp = JSON.stringify(next);
    if (fp === lastFingerprint) return;
    applySnapshot(next);
    render();
  }, 1000);
}

main();
