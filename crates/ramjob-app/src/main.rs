// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod commands;
mod state;

use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};

use ramjob_core::adaptive::next_sleep;
use ramjob_core::accountant::group_footprint;
use ramjob_core::fsm::{GroupFsm, GroupPhase};
use ramjob_core::gate::phys_memory;
use ramjob_core::grouper::AppGroup;
use ramjob_core::panel::PanelGroup;
use ramjob_core::policy::SystemArm;
use ramjob_core::sys_history::SysSample;

use state::{AppState, AppStateInner};

/// Tray menu pause item — shared between tray handler and `pause_all` IPC.
pub struct TrayPauseItem(pub MenuItem<tauri::Wry>);

pub fn sync_pause_menu_label(item: &MenuItem<tauri::Wry>, pause_all: bool) {
    let _ = item.set_text(if pause_all { "Resume" } else { "Pause all" });
}

fn show_panel(window: &tauri::WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn phase_str(phase: GroupPhase) -> &'static str {
    match phase {
        GroupPhase::Idle => "Idle",
        GroupPhase::Pressure => "Pressure",
        GroupPhase::Trim => "Trim",
        GroupPhase::LowYield => "LowYield",
        GroupPhase::Thrashing => "Thrashing",
    }
}

fn basename(key: &str) -> String {
    key.rsplit(['\\', '/']).next().unwrap_or(key).to_string()
}

/// Build the panel's group list from live enumeration + FSM phases.
///
/// Do **not** apply the CLI ≥50 MB GF floor here — the WebView filters that
/// for the default view and "Show all apps" must still receive sub-floor
/// groups (SPEC §7.2). Caps/always_enforce come from config when present.
fn build_panel_groups(
    apps: &[AppGroup],
    fsms: &HashMap<String, GroupFsm>,
    config: &ramjob_core::config::RamjobConfig,
) -> Vec<PanelGroup> {
    apps.iter()
        .map(|app| {
            let gf = group_footprint(app);
            let gc = config.groups.iter().find(|g| g.key == app.group_key);
            let fsm_hint = fsms
                .get(&app.group_key)
                .map(|f| phase_str(f.phase))
                .unwrap_or("Idle")
                .to_string();
            PanelGroup {
                key: app.group_key.clone(),
                name: basename(&app.group_key),
                gf_bytes: gf,
                cap_bytes: gc.map(|g| g.cap_bytes).unwrap_or(0),
                always_enforce: gc.map(|g| g.always_enforce).unwrap_or(false),
                fsm_hint,
            }
        })
        .collect()
}

/// Build a live tray tooltip from current used/total bytes and Armed/Idle
/// state — the tray icon is the only visible surface while the panel is
/// closed, so it should reflect state rather than a static string.
fn tray_tooltip(used: u64, total: u64, armed: bool, warning: bool) -> String {
    let used_gb = used as f64 / (1024.0 * 1024.0 * 1024.0);
    let total_gb = total as f64 / (1024.0 * 1024.0 * 1024.0);
    let state = if warning {
        "Warning"
    } else if armed {
        "Armed"
    } else {
        "Idle"
    };
    format!("RamJob — {used_gb:.1}/{total_gb:.1} GB — {state}")
}

/// One tick: `Runtime::tick` (pressure + enumerate + FSM), history sample,
/// panel group cache, and tray tooltip refresh.
fn run_tick(state: &AppState, app_handle: &AppHandle) {
    let now = Instant::now();

    let tooltip = {
        let Ok(mut guard) = state.0.lock() else {
            return;
        };
        let AppStateInner {
            runtime,
            pressure,
            panel,
            last_used_bytes,
            last_total_bytes,
            last_groups,
        } = &mut *guard;
        let config = panel.config.clone();
        let Ok(outcome) = runtime.tick(&config, pressure.as_mut(), now) else {
            return;
        };

        if let Ok((total, avail)) = phys_memory() {
            let used = total.saturating_sub(avail);
            *last_total_bytes = total;
            *last_used_bytes = used;
            panel.history.push_sample(SysSample {
                unix_ms: now_unix_ms(),
                used_bytes: used,
                total_bytes: total,
            });
        }

        *last_groups = build_panel_groups(&outcome.apps, &runtime.groups, &config);

        let warning = last_groups
            .iter()
            .any(|g| matches!(g.fsm_hint.as_str(), "LowYield" | "Thrashing"));
        let armed = runtime.policy.arm == SystemArm::Armed;
        tray_tooltip(*last_used_bytes, *last_total_bytes, armed, warning)
    };

    if let Some(tray) = app_handle.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(&tooltip));
    }
}

const TRAY_ID: &str = "main-tray";

/// Background loop: always runs `Runtime::tick`. Panel open → 1 s cadence;
/// closed → adaptive ladder per SPEC §6.1 (arm + hottest group phase).
fn spawn_tick_loop(app_handle: AppHandle) {
    std::thread::spawn(move || {
        loop {
            let panel_open = app_handle
                .get_webview_window("main")
                .map(|w| w.is_visible().unwrap_or(false))
                .unwrap_or(false);

            let state = app_handle.state::<AppState>();
            run_tick(state.inner(), &app_handle);

            let sleep = {
                let guard = state.inner().0.lock().ok();
                match guard {
                    Some(inner) => next_sleep(
                        inner.runtime.policy.arm,
                        inner.runtime.hottest_group_phase(),
                        panel_open,
                        inner.runtime.backstop_active(),
                    ),
                    None => next_sleep(SystemArm::Disarmed, None, panel_open, false),
                }
            };
            std::thread::sleep(sleep);
        }
    });
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::set_cap,
            commands::set_overall_limit,
            commands::set_flags,
            commands::pause_all,
            commands::copy_diagnostics,
        ])
        .setup(|app| {
            let app_state = AppState::new().expect("load RamJob config");
            app.manage(app_state);
            spawn_tick_loop(app.handle().clone());

            let pause_item = MenuItem::with_id(app, "pause", "Pause all", true, None::<&str>)?;
            app.manage(TrayPauseItem(pause_item.clone()));
            let open_item = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
            let settings_item =
                MenuItem::with_id(app, "settings", "Settings", false, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &pause_item,
                    &open_item,
                    &settings_item,
                    &PredefinedMenuItem::separator(app)?,
                    &quit_item,
                ],
            )?;

            // Reuse the app's bundled icon.ico instead of shipping a second
            // image asset — Tauri accepts it directly as the tray icon.
            let icon = app
                .default_window_icon()
                .cloned()
                .expect("bundle icon configured in tauri.conf.json");

            TrayIconBuilder::with_id(TRAY_ID)
                .icon(icon)
                .tooltip("RamJob — Idle")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    let state = app.state::<AppState>();
                    match event.id().as_ref() {
                        "pause" => {
                            if let Ok(mut inner) = state.inner().0.lock() {
                                let next = !inner.panel.config.pause_all;
                                match inner.panel.set_pause_all(next) {
                                    Ok(()) => {
                                        if let Some(tray_pause) =
                                            app.try_state::<TrayPauseItem>()
                                        {
                                            sync_pause_menu_label(&tray_pause.0, next);
                                        }
                                    }
                                    Err(e) => {
                                        inner
                                            .runtime
                                            .diagnostics
                                            .push(format!("set_pause_all failed: {e}"));
                                    }
                                }
                            }
                        }
                        "open" => {
                            if let Some(window) = app.get_webview_window("main") {
                                show_panel(&window);
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            show_panel(&window);
                        }
                    }
                })
                .build(app)?;

            // Hide the panel on focus loss (lazy show/hide instead of a
            // destroy-and-recreate cycle; cheap enough for a 420x600 webview).
            let app_handle = app.handle().clone();
            if let Some(window) = app.get_webview_window("main") {
                let handle = app_handle.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::Focused(false) => {
                        if let Some(w) = handle.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        if let Some(w) = handle.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                    _ => {}
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running RamJob tauri application");
}
