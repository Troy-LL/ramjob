// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod commands;
mod state;

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};

use ramjob_core::accountant::{group_footprint, meets_gf_floor};
use ramjob_core::fsm::{GroupFsm, GroupPhase};
use ramjob_core::grouper::{group_processes, AppGroup};
use ramjob_core::panel::PanelGroup;
use ramjob_core::scanner::{enumerate_processes_with_cache, PathCache};
use ramjob_core::sys_history::SysSample;

use state::{AppState, AppStateInner};

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

/// Total/used physical RAM via `GlobalMemoryStatusEx` (same API `ramjob-core`'s
/// gate module already links against — no new dependency).
fn system_memory() -> Result<(u64, u64), String> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    unsafe {
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        GlobalMemoryStatusEx(&mut status).map_err(|e| format!("GlobalMemoryStatusEx: {e}"))?;
        let total = status.ullTotalPhys;
        let used = total.saturating_sub(status.ullAvailPhys);
        Ok((total, used))
    }
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

/// Build the panel's group list from live enumeration + FSM phases, applying
/// the same visible-GF floor as the CLI (SPEC §5.2/§6). Caps/always_enforce
/// come from config when a group has one; groups not yet configured show a
/// zero cap so the UI can offer to set one.
fn build_panel_groups(
    apps: &[AppGroup],
    fsms: &HashMap<String, GroupFsm>,
    config: &ramjob_core::config::RamjobConfig,
) -> Vec<PanelGroup> {
    apps.iter()
        .filter_map(|app| {
            let gf = group_footprint(app);
            if !meets_gf_floor(gf) {
                return None;
            }
            let gc = config.groups.iter().find(|g| g.key == app.group_key);
            let fsm_hint = fsms
                .get(&app.group_key)
                .map(|f| phase_str(f.phase))
                .unwrap_or("Idle")
                .to_string();
            Some(PanelGroup {
                key: app.group_key.clone(),
                name: basename(&app.group_key),
                gf_bytes: gf,
                cap_bytes: gc.map(|g| g.cap_bytes).unwrap_or(0),
                always_enforce: gc.map(|g| g.always_enforce).unwrap_or(false),
                fsm_hint,
                honest: None,
            })
        })
        .collect()
}

/// One tick: sample pressure, enumerate processes, run the FSM/enforcer step,
/// record a system-memory history sample, and refresh the cached snapshot
/// inputs commands read from.
fn run_tick(inner: &mut AppStateInner, path_cache: &mut PathCache) {
    let now = Instant::now();
    let Ok(sample) = inner.pressure.sample() else {
        return;
    };
    let system = inner.runtime.policy.update(sample);

    let Ok(procs) = enumerate_processes_with_cache(path_cache) else {
        return;
    };
    let apps = group_processes(&procs);

    let _ = inner.runtime.tick_with_groups(system, &apps, now);

    if let Ok((total, used)) = system_memory() {
        inner.last_total_bytes = total;
        inner.last_used_bytes = used;
        inner.panel.history.push_sample(SysSample {
            unix_ms: now_unix_ms(),
            used_bytes: used,
            total_bytes: total,
        });
    }

    inner.last_groups = build_panel_groups(&apps, &inner.runtime.groups, &inner.panel.config);
}

/// Runs only while the panel window is visible (ponytail: plain background
/// thread + 1s sleep — no async runtime needed for a single poll loop).
fn spawn_tick_loop(app_handle: AppHandle) {
    std::thread::spawn(move || {
        let mut path_cache = PathCache::new();
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let Some(window) = app_handle.get_webview_window("main") else {
                continue;
            };
            if !window.is_visible().unwrap_or(false) {
                continue;
            }
            let state = app_handle.state::<AppState>();
            let Ok(mut inner) = state.0.lock() else {
                continue;
            };
            run_tick(&mut inner, &mut path_cache);
        }
    });
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::set_cap,
            commands::set_overall_limit,
            commands::pause_all,
            commands::copy_diagnostics,
        ])
        .setup(|app| {
            let app_state = AppState::new().expect("load RamJob config");
            app.manage(app_state);
            spawn_tick_loop(app.handle().clone());

            let pause_item = MenuItem::with_id(app, "pause", "Pause all", true, None::<&str>)?;
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

            TrayIconBuilder::new()
                .icon(icon)
                .tooltip("RamJob — Idle")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    let state = app.state::<AppState>();
                    match event.id().as_ref() {
                        "pause" => {
                            if let Ok(mut inner) = state.0.lock() {
                                let next = !inner.panel.config.pause_all;
                                if inner.panel.set_pause_all(next).is_ok() {
                                    inner.runtime.config.pause_all = next;
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
