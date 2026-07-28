// RamJob tray panel — shell wiring (Task 7).
// Task 8/9 fill in .history-chart / .hero-gauge / real app-card gauges.

const MIN_GF_BYTES = 50 * 1024 * 1024; // 50 MB "Show all apps" floor

// Mock PanelSnapshot (crates/ramjob-core/src/panel.rs) for layout iteration
// when opening index.html directly in a browser (no window.__TAURI__).
const MOCK_SNAPSHOT = {
  system_arm: "Disarmed", // "Armed" | "Disarmed"
  pause_all: false,
  used_bytes: 10 * 1024 * 1024 * 1024,
  total_bytes: 16 * 1024 * 1024 * 1024,
  overall_limit_bytes: 12 * 1024 * 1024 * 1024,
  status_line: "Idle — caps set but paused until memory gets tight",
  warning: false,
  samples: [],
  ceiling_edits: [],
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

async function main() {
  let snapshot = await fetchSnapshot();
  let showAll = false;

  const render = () => {
    renderPill(snapshot);
    renderStatusLine(snapshot);
    renderAppGrid(snapshot, showAll);
    renderPauseButton(snapshot);
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
