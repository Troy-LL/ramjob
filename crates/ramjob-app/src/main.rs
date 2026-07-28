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

use state::AppState;

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

/// Build a live tray tooltip from current used/total bytes and Armed/Idle
/// state — the tray icon is the only visible surface while the panel is
/// closed, so it should reflect state rather than a static string.
fn tray_tooltip(used: u64, total: u64, armed: bool) -> String {
    let used_gb = used as f64 / (1024.0 * 1024.0 * 1024.0);
    let total_gb = total as f64 / (1024.0 * 1024.0 * 1024.0);
    let state = if armed { "Armed" } else { "Idle" };
    format!("RamJob — {used_gb:.1}/{total_gb:.1} GB — {state}")
}

/// One tick: sample pressure, enumerate processes, run the FSM/enforcer step,
/// record a system-memory history sample, and refresh the cached snapshot
/// inputs commands read from.
///
/// The state lock is only held for the short pressure-sample/FSM steps —
/// `enumerate_processes_with_cache`/`group_processes` (which don't touch
/// shared state) run unlocked in between, so a slow enumeration can't stall
/// an IPC command (get_snapshot, set_cap, ...) waiting on the same mutex.
fn run_tick(state: &AppState, path_cache: &mut PathCache, app_handle: &AppHandle) {
    let now = Instant::now();

    let (system, config) = {
        let Ok(mut inner) = state.0.lock() else {
            return;
        };
        let Ok(sample) = inner.pressure.sample() else {
            return;
        };
        let system = inner.runtime.policy.update(sample);
        (system, inner.panel.config.clone())
    };

    let Ok(procs) = enumerate_processes_with_cache(path_cache) else {
        return;
    };
    let apps = group_processes(&procs);

    let Ok(mut inner) = state.0.lock() else {
        return;
    };
    let _ = inner.runtime.tick_with_groups(&config, system, &apps, now);

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

    let tooltip = tray_tooltip(
        inner.last_used_bytes,
        inner.last_total_bytes,
        inner.runtime.policy.arm == ramjob_core::policy::SystemArm::Armed,
    );
    drop(inner);
    if let Some(tray) = app_handle.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(&tooltip));
    }
}

const TRAY_ID: &str = "main-tray";

/// Runs only while the panel window is visible. Note: this means
/// `ramjob-app` only enforces caps while its panel/tray process is running
/// and the window is shown — it does not enforce while closed, and this
/// process is not a substitute for the `ramjob run` CLI daemon. See
/// "Known limitations" in .superpowers/sdd/m3-verify.md for the interaction
/// between panel-driven config edits and an already-running CLI daemon.
/// (ponytail: plain background thread + 1s sleep — no async runtime needed
/// for a single poll loop.)
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
            run_tick(&state, &mut path_cache, &app_handle);
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

            let pause_item_for_menu = pause_item.clone();
            TrayIconBuilder::with_id(TRAY_ID)
                .icon(icon)
                .tooltip("RamJob — Idle")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    let state = app.state::<AppState>();
                    match event.id().as_ref() {
                        "pause" => {
                            if let Ok(mut inner) = state.0.lock() {
                                let next = !inner.panel.config.pause_all;
                                match inner.panel.set_pause_all(next) {
                                    Ok(()) => {
                                        let _ = pause_item_for_menu.set_text(if next {
                                            "Resume"
                                        } else {
                                            "Pause all"
                                        });
                                    }
                                    Err(e) => {
                                        // Config write failed — leave the tray
                                        // label as-is (don't flip UI state on a
                                        // failed persist) and record the error
                                        // for `copy_diagnostics` instead of
                                        // silently discarding it.
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
