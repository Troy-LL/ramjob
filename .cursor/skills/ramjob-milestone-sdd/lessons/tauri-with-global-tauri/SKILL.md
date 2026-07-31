---
name: tauri-with-global-tauri
description: >-
  Vanilla Tauri v2 frontends that call window.__TAURI__ must set
  app.withGlobalTauri true, else production silently uses mock data.
  Use when RamJob panel shows fake Chrome/Slack apps or invoke never hits Rust.
---

# Tauri withGlobalTauri

Leading word: **global invoke**.

## When

- Panel lists mock apps (Google Chrome, Slack, …) while real processes differ.
- Cap/pause/ceiling edits do not change `%APPDATA%\RamJob\config.toml`.
- `app.js` uses `window.__TAURI__.core` / `.tauri` without a bundler.

## Steps

1. Confirm `crates/ramjob-app/tauri.conf.json` has `"withGlobalTauri": true` under `app`.
2. Rebuild `ramjob-app` after the config change (config is baked at build).
3. In WebView DevTools, `window.__TAURI__` must be defined; if not, the UI path is still mock.
4. Prefer one `ipc(cmd, args)` helper over per-call `__TAURI__` probes (less silent fallback risk).

**Done when:** a live `get_snapshot` returns real group keys from the machine, not `MOCK_SNAPSHOT` names.
