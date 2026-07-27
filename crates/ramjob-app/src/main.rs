// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

/// Stub shared state for the tray "Pause all" toggle until Task 6 wires the
/// real `PanelState` + IPC.
/// ponytail: single global flag, replace with PanelState in Task 6.
struct PauseState(Arc<AtomicBool>);

fn show_panel(window: &tauri::WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let pause = Arc::new(AtomicBool::new(false));
            app.manage(PauseState(pause));

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
                    let state = app.state::<PauseState>();
                    match event.id().as_ref() {
                        "pause" => {
                            let paused = !state.0.load(Ordering::SeqCst);
                            state.0.store(paused, Ordering::SeqCst);
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
                    if let TrayIconEvent::Click { .. } = event {
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
