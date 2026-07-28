//! Tauri IPC commands for the panel UI (Task 6).
//!
//! All mutation goes through `PanelState`'s existing mutators — never
//! duplicated here — so the "crossing the overall ceiling never arms
//! anything" invariant stays enforced in exactly one place.

use std::time::{SystemTime, UNIX_EPOCH};

use tauri::State;

use ramjob_core::panel::PanelSnapshot;

use crate::clipboard::set_clipboard_text;
use crate::state::AppState;

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[tauri::command]
pub fn get_snapshot(state: State<AppState>) -> Result<PanelSnapshot, String> {
    let inner = state.0.lock().map_err(|_| "state poisoned".to_string())?;
    Ok(inner.panel.build_snapshot(
        inner.runtime.policy.arm,
        inner.last_used_bytes,
        inner.last_total_bytes,
        &inner.last_groups,
    ))
}

#[tauri::command]
pub fn set_cap(
    state: State<AppState>,
    key: String,
    cap_bytes: u64,
    shift_fine: bool,
) -> Result<PanelSnapshot, String> {
    let mut inner = state.0.lock().map_err(|_| "state poisoned".to_string())?;
    let median_gf = inner
        .last_groups
        .iter()
        .find(|g| g.key == key)
        .map(|g| g.gf_bytes);
    inner.panel.set_cap(&key, cap_bytes, shift_fine, median_gf)?;
    let (arm, used, total) = (
        inner.runtime.policy.arm,
        inner.last_used_bytes,
        inner.last_total_bytes,
    );
    let groups = inner.last_groups.clone();
    Ok(inner.panel.build_snapshot(arm, used, total, &groups))
}

#[tauri::command]
pub fn set_overall_limit(
    state: State<AppState>,
    limit_bytes: u64,
    shift_fine: bool,
) -> Result<PanelSnapshot, String> {
    let mut inner = state.0.lock().map_err(|_| "state poisoned".to_string())?;
    inner
        .panel
        .set_overall_limit(limit_bytes, now_unix_ms(), shift_fine)?;
    let (arm, used, total) = (
        inner.runtime.policy.arm,
        inner.last_used_bytes,
        inner.last_total_bytes,
    );
    let groups = inner.last_groups.clone();
    Ok(inner.panel.build_snapshot(arm, used, total, &groups))
}

#[tauri::command]
pub fn pause_all(state: State<AppState>, pause: bool) -> Result<PanelSnapshot, String> {
    let mut inner = state.0.lock().map_err(|_| "state poisoned".to_string())?;
    inner.panel.set_pause_all(pause)?;
    let (arm, used, total) = (
        inner.runtime.policy.arm,
        inner.last_used_bytes,
        inner.last_total_bytes,
    );
    let groups = inner.last_groups.clone();
    Ok(inner.panel.build_snapshot(arm, used, total, &groups))
}

#[tauri::command]
pub fn copy_diagnostics(state: State<AppState>) -> Result<(), String> {
    let inner = state.0.lock().map_err(|_| "state poisoned".to_string())?;
    let text = inner.panel.diagnostics_text(&inner.runtime.diagnostics);
    set_clipboard_text(&text)
}
