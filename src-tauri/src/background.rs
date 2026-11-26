use std::sync::atomic::Ordering;
use std::time::Duration;

use manka_core::autoselect::{self, AutoRequest, AutoState};
use manka_core::repository;
use tauri::{Emitter, Manager};

use crate::engine::bring_engine_up;
use crate::state::{app_log, persist_settings, settings_of, CoreState};
use crate::tray::refresh_tray;

/// Background loop: scheduled list refresh + the self-healing watchdog that
/// re-runs autoselection when the active strategy degrades.
pub(crate) fn spawn_background_tasks(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_cf_refresh = manka_core::util::now_ms();
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let state = app.state::<CoreState>();
            let settings = settings_of(&state);
            let now = manka_core::util::now_ms();

            // ---- Cloudflare proxy domains rotate upstream; refresh them twice
            // a day so the Telegram proxy keeps a desynced route.
            if now - last_cf_refresh >= 12 * 3_600_000 {
                last_cf_refresh = now;
                if manka_core::lists::refresh_tgws_domains(&state.paths).await {
                    app_log(&state, "telegram proxy domain list updated");
                    if state.processes.engine_running().await {
                        let _ = bring_engine_up(&state).await;
                    }
                    if settings.tgws_enabled {
                        let _ = crate::engine::tgws_restart_impl(&state).await;
                    }
                    let _ = app.emit("lists-updated", ());
                }
            }

            // ---- scheduled list refresh
            if settings.lists_auto_update
                && now - settings.last_lists_update
                    >= (settings.lists_update_interval_hours.max(1) as i64) * 3_600_000
            {
                let report = manka_core::lists::update(&state.paths, &settings).await;
                let mut s = settings_of(&state);
                s.last_lists_update = now;
                let _ = persist_settings(&state, &s);
                if report.sources_ok > 0 {
                    app_log(
                        &state,
                        &format!(
                            "lists auto-update: {} domains, {} failed source(s)",
                            report.domains, report.sources_failed
                        ),
                    );
                    let _ = app.emit("lists-updated", &report);
                }
            }

            // ---- watchdog
            if !settings.watchdog_enabled || !settings.enabled {
                continue;
            }
            if now - settings.last_watchdog_check
                < (settings.watchdog_interval_min.max(5) as i64) * 60_000
            {
                continue;
            }
            // never race with a user-driven autoselection
            let Ok(_guard) = state.auto_lock.try_lock() else {
                continue;
            };
            if !state.processes.engine_running().await {
                continue;
            }
            let report = manka_core::watchdog::check(&settings).await;
            {
                let mut s = settings_of(&state);
                s.last_watchdog_check = report.checked_at;
                let _ = persist_settings(&state, &s);
            }
            app_log(
                &state,
                &format!(
                    "watchdog: healthy={} percent={} — {}",
                    report.healthy, report.percent, report.detail
                ),
            );
            let _ = app.emit("watchdog", &report);
            if report.healthy || !settings.watchdog_auto_fix || !manka_core::util::is_elevated() {
                continue;
            }

            app_log(&state, "watchdog: starting auto-fix (autoselect)");
            state.cancel.store(false, Ordering::Relaxed);
            let request = AutoRequest {
                engines: vec![settings.engine],
                targets: settings.all_targets(),
                full: false,
                include_store: settings.autoselect_include_store,
                requests: settings.autoselect_requests,
                timeout_sec: settings.autoselect_timeout,
                stop_after_perfect: settings.autoselect_stop_after_perfect,
            };
            let strategies = repository::all(&state.paths);
            let app_progress = app.clone();
            let progress = move |s: &AutoState| {
                let _ = app_progress.emit("auto-progress", s);
            };
            let result = autoselect::run(
                &state.paths,
                &settings,
                &strategies,
                &state.processes,
                &request,
                &state.cancel,
                progress,
            )
            .await;
            let applied = result.best().filter(|b| b.percent > 0).cloned();
            match applied {
                Some(best) => {
                    if let Ok(saved) = repository::save_auto(
                        &state.paths,
                        &best.name,
                        best.engine,
                        &best.template,
                        best.percent,
                    ) {
                        let mut s = settings_of(&state);
                        s.set_active(saved.engine, &saved.id);
                        s.engine = saved.engine;
                        s.last_check = manka_core::util::now_ms();
                        s.last_check_rate = best.percent as i64;
                        let _ = persist_settings(&state, &s);
                        let _ = bring_engine_up(&state).await;
                        app_log(
                            &state,
                            &format!("watchdog: applied «{}» ({}%)", saved.name, best.percent),
                        );
                        let _ = app.emit(
                            "watchdog-fixed",
                            serde_json::json!({ "name": saved.name, "percent": best.percent }),
                        );
                    }
                }
                None => {
                    // nothing better found: restore the previous strategy
                    let _ = bring_engine_up(&state).await;
                    app_log(&state, "watchdog: auto-fix found nothing better");
                    let _ = app.emit("watchdog-failed", ());
                }
            }
            refresh_tray(&app).await;
            let _ = app.emit("status-changed", ());
        }
    });
}
