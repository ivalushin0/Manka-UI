use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager};

use crate::engine::{
    bring_engine_down, bring_engine_up, start_proxy_impl, stop_everything, stop_proxy_impl,
};
use crate::state::{app_log, settings_of, CoreState, TraySnapshot};

const TRAY_ICON_ON: &[u8] = include_bytes!("../icons/32x32.png");
const TRAY_ICON_OFF: &[u8] = include_bytes!("../icons/tray-off.png");

fn tray_image(healthy: bool) -> tauri::Result<Image<'static>> {
    Image::from_bytes(if healthy { TRAY_ICON_ON } else { TRAY_ICON_OFF })
}

async fn tray_snapshot(state: &CoreState) -> TraySnapshot {
    let settings = settings_of(state);
    let engine = state
        .processes
        .engine_status(&state.paths.logs().join("engine.log"))
        .await;
    let proxy = state
        .processes
        .proxy_status(&state.paths.logs().join("tgws.log"))
        .await;

    let healthy = settings.enabled && engine.running && (!settings.tgws_enabled || proxy.running);

    let engine_text = if settings.enabled {
        if engine.running {
            "Обход интернета — включён"
        } else {
            "Обход интернета — ошибка запуска"
        }
    } else {
        "Обход интернета — выключен"
    };
    let proxy_text = if settings.tgws_enabled {
        if proxy.running {
            "Обход Telegram — включён"
        } else {
            "Обход Telegram — ошибка запуска"
        }
    } else {
        "Обход Telegram — выключен"
    };

    let reason = if !settings.enabled {
        "обход интернета выключен"
    } else if !engine.running {
        "движок обхода не запущен"
    } else if settings.tgws_enabled && !proxy.running {
        "прокси Telegram не запущен"
    } else if settings.tgws_enabled {
        "обход и Telegram работают"
    } else {
        "обход работает, Telegram выключен"
    };

    TraySnapshot {
        healthy,
        tooltip: if healthy {
            format!("Manka — {reason}")
        } else {
            format!("Manka — не работает: {reason}")
        },
        engine_text: engine_text.to_string(),
        proxy_text: proxy_text.to_string(),
        engine_checked: settings.enabled,
        proxy_checked: settings.tgws_enabled,
    }
}

fn build_tray_menu<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    snap: &TraySnapshot,
) -> tauri::Result<Menu<R>> {
    let open = MenuItem::with_id(app, "open", "Открыть Manka", true, None::<&str>)?;
    let internet = CheckMenuItem::with_id(
        app,
        "toggle_internet",
        &snap.engine_text,
        true,
        snap.engine_checked,
        None::<&str>,
    )?;
    let telegram = CheckMenuItem::with_id(
        app,
        "toggle_telegram",
        &snap.proxy_text,
        true,
        snap.proxy_checked,
        None::<&str>,
    )?;
    let pause = MenuItem::with_id(
        app,
        "pause",
        "Остановить обход — пауза",
        snap.engine_checked || snap.proxy_checked,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
    let sep_top = PredefinedMenuItem::separator(app)?;
    let sep_bottom = PredefinedMenuItem::separator(app)?;
    Menu::with_items(
        app,
        &[
            &open,
            &sep_top,
            &internet,
            &telegram,
            &sep_bottom,
            &pause,
            &quit,
        ],
    )
}

fn apply_tray(app: &tauri::AppHandle, snap: TraySnapshot) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(tray) = handle.tray_by_id("main") else {
            return;
        };
        if let Ok(menu) = build_tray_menu(&handle, &snap) {
            let _ = tray.set_menu(Some(menu));
        }
        if let Ok(icon) = tray_image(snap.healthy) {
            let _ = tray.set_icon(Some(icon));
        }
        let _ = tray.set_tooltip(Some(snap.tooltip.as_str()));
    });
}

/// Recompute tray icon/menu. Cheap enough to call after every engine/proxy
/// action and from the polling watcher that catches crashes.
pub(crate) async fn refresh_tray(app: &tauri::AppHandle) {
    let state = app.state::<CoreState>();
    let snap = tray_snapshot(&state).await;
    let changed = match state.tray.lock() {
        Ok(mut last) => {
            if last.as_ref() == Some(&snap) {
                false
            } else {
                *last = Some(snap.clone());
                true
            }
        }
        Err(_) => true,
    };
    if changed {
        apply_tray(app, snap);
    }
}

pub(crate) fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let settings = settings_of(&app.state::<CoreState>());
    let initial = TraySnapshot {
        healthy: settings.enabled,
        tooltip: "Manka".to_string(),
        engine_text: if settings.enabled {
            "Обход интернета — включён".to_string()
        } else {
            "Обход интернета — выключен".to_string()
        },
        proxy_text: if settings.tgws_enabled {
            "Обход Telegram — включён".to_string()
        } else {
            "Обход Telegram — выключен".to_string()
        },
        engine_checked: settings.enabled,
        proxy_checked: settings.tgws_enabled,
    };
    let menu = build_tray_menu(app.handle(), &initial)?;
    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Manka")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "toggle_internet" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<CoreState>();
                    if settings_of(&state).enabled {
                        let _ = bring_engine_down(&state).await;
                    } else if let Err(e) = bring_engine_up(&state).await {
                        app_log(&state, &format!("tray: internet start failed: {e}"));
                    }
                    refresh_tray(&app).await;
                    let _ = app.emit("status-changed", ());
                });
            }
            "toggle_telegram" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<CoreState>();
                    if settings_of(&state).tgws_enabled {
                        let _ = stop_proxy_impl(&state).await;
                    } else if let Err(e) = start_proxy_impl(&state).await {
                        app_log(&state, &format!("tray: telegram start failed: {e}"));
                    }
                    refresh_tray(&app).await;
                    let _ = app.emit("status-changed", ());
                });
            }
            "pause" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<CoreState>();
                    stop_everything(&state).await;
                    refresh_tray(&app).await;
                    let _ = app.emit("status-changed", ());
                });
            }
            "quit" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<CoreState>();
                    stop_everything(&state).await;
                    app.exit(0);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
    if let Ok(icon) = tray_image(initial.healthy) {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}
