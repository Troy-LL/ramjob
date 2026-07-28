// RamJob tray panel — shell wiring (Task 7).
// Task 8/9 fill in .history-chart / .hero-gauge / real app-card gauges.

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
    { key: "chrome", name: "Google Chrome", gf_bytes: 2.1 * 1024 * 1024 * 1024, cap_bytes: 4 * 1024 * 1024 * 1024, always_enforce: false, fsm_hint: "Idle", honest: null },
    { key: "slack", name: "Slack", gf_bytes: 900 * 1024 * 1024, cap_bytes: 0, always_enforce: false, fsm_hint: "Idle", honest: null },
    { key: "vscode", name: "Visual Studio Code", gf_bytes: 1.3 * 1024 * 1024 * 1024, cap_bytes: 0, always_enforce: false, fsm_hint: "Pressure", honest: null },
    { key: "docker", name: "Docker Desktop", gf_bytes: 3.4 * 1024 * 1024 * 1024, cap_bytes: 6 * 1024 * 1024 * 1024, always_enforce: true, fsm_hint: "Idle", honest: null },
    { key: "spotify", name: "Spotify", gf_bytes: 30 * 1024 * 1024, cap_bytes: 0, always_enforce: false, fsm_hint: "Idle", honest: null },
    { key: "figma", name: "Figma", gf_bytes: 420 * 1024 * 1024, cap_bytes: 0, always_enforce: false, fsm_hint: "Idle", honest: null },
  ],
};

const isTauri = () => typeof window !== "undefined" && !!window.__TAURI__;

async function fetchSnapshot() {
  if (isTauri()) {
    const { invoke } = window.__TAURI__.core ?? window.__TAURI__.tauri;
    return invoke("get_snapshot");
  }
  return MOCK_SNAPSHOT;
}

async function callPauseAll(pause) {
  if (isTauri()) {
    const { invoke } = window.__TAURI__.core ?? window.__TAURI__.tauri;
    return invoke("pause_all", { pause });
  }
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

async function callSetOverallLimit(limitBytes, shiftFine) {
  if (isTauri()) {
    const { invoke } = window.__TAURI__.core ?? window.__TAURI__.tauri;
    return invoke("set_overall_limit", { limitBytes, shiftFine });
  }
  const snapped = snapCapBytesPreview(limitBytes, shiftFine);
  MOCK_SNAPSHOT.overall_limit_bytes = snapped;
  MOCK_SNAPSHOT.ceiling_edits.push({ unix_ms: Date.now(), overall_limit_bytes: snapped });
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

function renderAppGrid(snapshot, showAll) {
  const grid = document.getElementById("app-grid");
  grid.innerHTML = "";

  const groups = [...snapshot.groups].sort((a, b) => b.gf_bytes - a.gf_bytes);
  const hidden = groups.filter((g) => g.gf_bytes < MIN_GF_BYTES);
  document.getElementById("hidden-count").textContent = hidden.length;

  const visible = showAll ? groups : groups.filter((g) => g.gf_bytes >= MIN_GF_BYTES);

  for (const g of visible) {
    const card = document.createElement("div");
    card.className = "app-card";
    card.innerHTML = `
      <div class="app-name">${g.name}</div>
      <div class="app-gauge-placeholder"></div>
      <div class="app-meta">${formatBytes(g.gf_bytes)}${g.cap_bytes ? " / cap " + formatBytes(g.cap_bytes) : ""}</div>
    `;
    grid.appendChild(card);
  }
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
    handle.setPointerCapture(ev.pointerId);
    dragState.active = true;
    dragState.previewBytes = snapshot.overall_limit_bytes;

    const onMove = (moveEv) => {
      dragState.previewBytes = bytesFromY(moveEv.clientY);
      renderHistoryChart(snapshot, onCommitLimit); // live preview only, no IPC
    };
    const onUp = async (upEv) => {
      handle.removeEventListener("pointermove", onMove);
      handle.removeEventListener("pointerup", onUp);
      const finalBytes = dragState.previewBytes;
      const shiftFine = upEv.shiftKey;
      dragState.active = false;
      await onCommitLimit(finalBytes, shiftFine); // commits + refreshes snapshot
    };
    handle.addEventListener("pointermove", onMove);
    handle.addEventListener("pointerup", onUp);
  });

  container.appendChild(svg);
}

async function main() {
  let snapshot = await fetchSnapshot();
  let showAll = false;

  const render = () => {
    renderPill(snapshot);
    renderStatusLine(snapshot);
    renderAppGrid(snapshot, showAll);
    renderPauseButton(snapshot);
    renderHistoryChart(snapshot, async (limitBytes, shiftFine) => {
      snapshot = await callSetOverallLimit(limitBytes, shiftFine);
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
    renderAppGrid(snapshot, showAll);
  });

  document.getElementById("pause-all-btn").addEventListener("click", async () => {
    snapshot = await callPauseAll(!snapshot.pause_all);
    renderPauseButton(snapshot);
    renderStatusLine(snapshot);
  });
}

main();
