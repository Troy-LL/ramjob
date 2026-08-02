//! Tauri IPC commands for the panel UI (Task 6).
//!
//! All mutation goes through `PanelState`'s existing mutators — never
//! duplicated here — so the "crossing the overall ceiling never arms
//! anything" invariant stays enforced in exactly one place.

use std::time::{SystemTime, UNIX_EPOCH};

use tauri::State;

use ramjob_core::panel::PanelSnapshot;

use crate::clipboard::set_clipboard_text;
use crate::state::{AppState, AppStateInner};
use crate::{sync_pause_menu_label, TrayPauseItem};

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn snapshot_from(inner: &AppStateInner) -> PanelSnapshot {
    inner.panel.build_snapshot(
        inner.runtime.policy.arm,
        inner.last_used_bytes,
        inner.last_total_bytes,
        &inner.last_groups,
    )
}

#[tauri::command]
pub fn get_snapshot(state: State<AppState>) -> Result<PanelSnapshot, String> {
    let inner = state.inner().0.lock().map_err(|_| "state poisoned".to_string())?;
    Ok(snapshot_from(&inner))
}

#[tauri::command]
pub fn set_cap(
    state: State<AppState>,
    key: String,
    cap_bytes: u64,
    shift_fine: bool,
) -> Result<PanelSnapshot, String> {
    let mut inner = state.inner().0.lock().map_err(|_| "state poisoned".to_string())?;
    // SPEC §7.5/§8.2 wants a real 24h median floor input; that histogram is
    // out of M3 scope, so pass None here to use apply_cap_floor's flat
    // 300MB (FLOOR_FLAT_BYTES) fallback rather than an instantaneous GF
    // sample (which a transient spike would wrongly bake in as "the median").
    inner.panel.set_cap(&key, cap_bytes, shift_fine, None)?;
    Ok(snapshot_from(&inner))
}

#[tauri::command]
pub fn set_overall_limit(
    state: State<AppState>,
    limit_bytes: u64,
    shift_fine: bool,
) -> Result<PanelSnapshot, String> {
    let mut inner = state.inner().0.lock().map_err(|_| "state poisoned".to_string())?;
    inner
        .panel
        .set_overall_limit(limit_bytes, now_unix_ms(), shift_fine)?;
    Ok(snapshot_from(&inner))
}

#[tauri::command]
pub fn set_flags(
    state: State<AppState>,
    key: String,
    always_enforce: bool,
) -> Result<PanelSnapshot, String> {
    let mut inner = state.inner().0.lock().map_err(|_| "state poisoned".to_string())?;
    inner.panel.set_flags(&key, always_enforce)?;
    Ok(snapshot_from(&inner))
}

#[tauri::command]
pub fn pause_all(
    state: State<AppState>,
    pause: bool,
    tray_pause: State<TrayPauseItem>,
) -> Result<PanelSnapshot, String> {
    let mut inner = state.inner().0.lock().map_err(|_| "state poisoned".to_string())?;
    inner.panel.set_pause_all(pause)?;
    sync_pause_menu_label(&tray_pause.0, pause);
    Ok(snapshot_from(&inner))
}

#[tauri::command]
pub fn copy_diagnostics(state: State<AppState>) -> Result<(), String> {
    let inner = state.inner().0.lock().map_err(|_| "state poisoned".to_string())?;
    let text = inner.panel.diagnostics_text(&inner.runtime.diagnostics);
    set_clipboard_text(&text)
}
