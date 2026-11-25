mod background;
mod commands;
mod engine;
mod state;
mod tray;
mod updater;

use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::Duration;

use manka_core::paths::Paths;
use manka_core::runner::ProcessManager;
use manka_core::settings::Settings;
use tauri::{Emitter, Manager};

pub use state::CoreState;

use crate::background::spawn_background_tasks;
use crate::commands::*;
use crate::engine::{bring_engine_up, start_proxy_impl, stop_everything};
use crate::state::{app_log, settings_of};
use crate::tray::{build_tray, refresh_tray};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = Paths::detect();
    let _ = paths.ensure();
    let _ = manka_core::settings::ensure_exclude_list(&paths);
    let _ = manka_core::settings::ensure_hostlist(&paths);
    let mut settings = Settings::load(&paths);
    if settings.tgws_secret.trim().is_empty() {
        settings.tgws_secret = Settings::default().tgws_secret;
    }
    let _ = settings.save(&paths);

    let state = CoreState {
        paths,
        settings: Mutex::new(settings),
        processes: ProcessManager::default(),
        cancel: AtomicBool::new(false),
        auto_lock: tokio::sync::Mutex::new(()),
        tray: Mutex::new(None),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state)
        .setup(|app| {
            // Only the primary instance reaches setup; a second launch just
            // focuses the running window. Cleaning stale components here keeps
            // a second launch from killing the live engine.
            if manka_core::util::is_elevated() {
                let state = app.state::<CoreState>();
                let killed = manka_core::util::kill_stale_components(&state.paths.components());
                if killed > 0 {
                    app_log(
                        &state,
                        &format!("cleaned up {killed} stale component process(es)"),
                    );
                }
            }
            build_tray(app)?;
            let start_hidden = settings_of(&app.state::<CoreState>()).start_minimized;
            if !start_hidden {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<CoreState>();
                let settings = settings_of(&state);
                if settings.enabled {
                    if manka_core::util::is_elevated() {
                        app_log(&state, "autostart: starting engine");
                        if let Err(e) = bring_engine_up(&state).await {
                            app_log(&state, &format!("autostart engine failed: {e}"));
                        }
                    } else {
                        app_log(&state, "autostart skipped: not elevated");
                    }
                }
                if settings.tgws_enabled {
                    if let Err(e) = start_proxy_impl(&state).await {
                        app_log(&state, &format!("autostart proxy failed: {e}"));
                    }
                }
                let _ = handle.emit("status-changed", ());
                refresh_tray(&handle).await;
            });
            // Keep the tray honest even when a child process dies on its own.
            let watcher = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(4)).await;
                    refresh_tray(&watcher).await;
                }
            });
            spawn_background_tasks(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let close_to_tray = app
                    .state::<CoreState>()
                    .settings
                    .lock()
                    .map(|s| s.close_to_tray)
                    .unwrap_or(false);
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    // closing for real: stop the child processes first so no
                    // orphaned winws keeps intercepting traffic
                    api.prevent_close();
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app.state::<CoreState>();
                        stop_everything(&state).await;
                        app.exit(0);
                    });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_status,
            port_owner,
            stop_port_owner,
            save_settings,
            set_enabled,
            engine_start,
            engine_stop,
            set_active_strategy,
            set_strategy_option,
            set_strategy_flag,
            catalog,
            install_component,
            check_updates,
            rollback_component,
            tgws_start,
            tgws_stop,
            tgws_check,
            tgws_regenerate_secret,
            tgws_link,
            store_refresh,
            store_items,
            store_install,
            store_remove,
            probe_targets,
            auto_start,
            auto_cancel,
            auto_apply,
            get_history,
            read_log,
            log_info,
            clear_log,
            is_elevated,
            relaunch_elevated,
            open_url,
            app_update_check,
            app_update_install,
            open_data_dir,
            open_logs_dir,
            set_autostart,
            delete_strategy,
            lists_update_now,
            doctor_run,
            doctor_export,
            watchdog_check_now,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Manka");
}
