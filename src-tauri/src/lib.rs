use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use manka_core::autoselect::{self, AutoRequest, AutoState};
use manka_core::model::{
    ComponentStatus, Engine, ProcessStatus, SiteResult, StoreItem, Strategy,
};
use manka_core::paths::Paths;
use manka_core::runner::ProcessManager;
use manka_core::settings::Settings;
use manka_core::{args, builtin, components, repository, store};
use serde::Serialize;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

pub struct CoreState {
    pub paths: Paths,
    pub settings: Mutex<Settings>,
    pub processes: ProcessManager,
    pub cancel: AtomicBool,
    pub auto_lock: tokio::sync::Mutex<()>,
    /// Last picture painted into the tray, so the polling watcher only redraws
    /// the icon/menu when something actually changed.
    tray: Mutex<Option<TraySnapshot>>,
}

#[derive(Clone, PartialEq)]
struct TraySnapshot {
    healthy: bool,
    tooltip: String,
    engine_text: String,
    proxy_text: String,
    engine_checked: bool,
    proxy_checked: bool,
}

#[derive(Serialize)]
struct TargetGroupDto {
    id: String,
    title: String,
    urls: Vec<String>,
}

#[derive(Serialize)]
struct KitDto {
    id: String,
    name: String,
    version: String,
}

#[derive(Serialize)]
struct AppDto {
    version: String,
    data_dir: String,
    elevated: bool,
    settings: Settings,
    strategies: Vec<Strategy>,
    engine: ProcessStatus,
    proxy: ProcessStatus,
    components: Vec<ComponentStatus>,
    target_groups: Vec<TargetGroupDto>,
    kits: Vec<KitDto>,
    history: Vec<manka_core::history::HistoryRun>,
    log_tail: Vec<String>,
}

fn settings_of(state: &CoreState) -> Settings {
    state.settings.lock().map(|s| s.clone()).unwrap_or_default()
}

fn persist_settings(state: &CoreState, settings: &Settings) -> Result<(), String> {
    settings.save(&state.paths).map_err(|e| e.to_string())?;
    if let Ok(mut slot) = state.settings.lock() {
        *slot = settings.clone();
    }
    Ok(())
}

fn app_log(state: &CoreState, line: &str) {
    manka_core::util::append_log(&state.paths.logs().join("app.log"), line);
}

async fn bring_engine_up(state: &CoreState) -> Result<ProcessStatus, String> {
    let mut settings = settings_of(state);
    let strategy = repository::active(&state.paths, &settings, settings.engine)
        .ok_or("Нет активной стратегии для этого движка")?;
    let kit = strategy.kit_id.as_ref().map(|id| state.paths.kit(id));
    let engine_dir = state.paths.engine_dir();
    let rendered = args::render(
        &strategy,
        &settings,
        &state.paths,
        engine_dir.as_deref(),
        kit.as_deref(),
    )?;
    let exe = rendered.exe.ok_or_else(|| {
        "Zapret не установлен. Откройте «Компоненты» и установите Zapret 2.".to_string()
    })?;
    let log = state.paths.logs().join("engine.log");
    state
        .processes
        .start_engine(&exe, &rendered.args, rendered.working_dir.as_deref(), &log)
        .await?;
    tokio::time::sleep(Duration::from_millis(900)).await;
    let status = state.processes.engine_status(&log).await;
    if !status.running {
        let tail = manka_core::util::tail_lines(&log, 10).join(" | ");
        return Err(if tail.is_empty() {
            "Движок завершился сразу после запуска. Запустите Manka от имени администратора.".into()
        } else {
            format!("Движок завершился: {tail}")
        });
    }
    settings.enabled = true;
    persist_settings(state, &settings)?;
    app_log(
        state,
        &format!("engine started: {} ({})", strategy.name, rendered.command_line),
    );
    Ok(status)
}

async fn bring_engine_down(state: &CoreState) -> Result<ProcessStatus, String> {
    let log = state.paths.logs().join("engine.log");
    state.processes.stop_engine(&log).await;
    let mut settings = settings_of(state);
    settings.enabled = false;
    persist_settings(state, &settings)?;
    app_log(state, "engine stopped");
    Ok(ProcessStatus::default())
}

async fn stop_everything(state: &CoreState) {
    let engine_log = state.paths.logs().join("engine.log");
    let proxy_log = state.paths.logs().join("tgws.log");
    state.processes.stop_all(&engine_log, &proxy_log).await;
    let mut settings = settings_of(state);
    settings.enabled = false;
    settings.tgws_enabled = false;
    let _ = persist_settings(state, &settings);
}

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
        &[&open, &sep_top, &internet, &telegram, &sep_bottom, &pause, &quit],
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
async fn refresh_tray(app: &tauri::AppHandle) {
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

#[tauri::command]
async fn get_state(state: State<'_, CoreState>) -> Result<AppDto, String> {
    let settings = settings_of(&state);
    let engine = state
        .processes
        .engine_status(&state.paths.logs().join("engine.log"))
        .await;
    let proxy = state
        .processes
        .proxy_status(&state.paths.logs().join("tgws.log"))
        .await;
    let components = components::status(&state.paths);
    let target_groups = manka_core::TARGET_GROUPS
        .iter()
        .map(|(id, title, urls)| TargetGroupDto {
            id: (*id).to_string(),
            title: (*title).to_string(),
            urls: urls.iter().map(|u| (*u).to_string()).collect(),
        })
        .collect();
    let kits = store::installed_kits_list(&state.paths)
        .into_iter()
        .map(|(id, name, version)| KitDto { id, name, version })
        .collect();
    let strategies = repository::all(&state.paths);
    let history = manka_core::history::load(&state.paths);
    let log_tail = manka_core::util::tail_lines(&state.paths.logs().join("app.log"), 200);
    Ok(AppDto {
        version: manka_core::VERSION.to_string(),
        data_dir: state.paths.root().to_string_lossy().to_string(),
        elevated: manka_core::util::is_elevated(),
        settings,
        strategies,
        engine,
        proxy,
        components,
        target_groups,
        kits,
        history,
        log_tail,
    })
}

#[derive(Serialize)]
struct StatusDto {
    engine: ProcessStatus,
    proxy: ProcessStatus,
}

#[tauri::command]
async fn get_status(state: State<'_, CoreState>) -> Result<StatusDto, String> {
    Ok(StatusDto {
        engine: state
            .processes
            .engine_status(&state.paths.logs().join("engine.log"))
            .await,
        proxy: state
            .processes
            .proxy_status(&state.paths.logs().join("tgws.log"))
            .await,
    })
}

#[derive(Serialize)]
struct PortOwnerDto {
    pid: u32,
    name: String,
}

#[tauri::command]
fn port_owner(port: u16) -> Option<PortOwnerDto> {
    manka_core::util::port_owner(port).map(|(pid, name)| PortOwnerDto { pid, name })
}

#[tauri::command]
fn stop_port_owner(port: u16) -> Result<String, String> {
    manka_core::util::stop_port_owner(port)
}

#[tauri::command]
async fn save_settings(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
    mut settings: Settings,
) -> Result<Settings, String> {
    if settings.tgws_secret.trim().is_empty() {
        settings.tgws_secret = Settings::default().tgws_secret;
    }
    settings.tgws_port = settings.tgws_port.max(1);
    let old = settings_of(&state);
    let engine_changed = old.engine != settings.engine;
    let proxy_changed = old.tgws_port != settings.tgws_port
        || old.tgws_secret != settings.tgws_secret
        || old.tgws_pool_size != settings.tgws_pool_size
        || old.tgws_cloudflare != settings.tgws_cloudflare
        || old.tgws_dc_ips != settings.tgws_dc_ips
        || old.tgws_extra != settings.tgws_extra
        || old.debug_logs != settings.debug_logs;
    persist_settings(&state, &settings)?;
    if engine_changed && settings.enabled {
        bring_engine_up(&state).await?;
    }
    if proxy_changed {
        let proxy = state
            .processes
            .proxy_status(&state.paths.logs().join("tgws.log"))
            .await;
        if proxy.running {
            start_proxy_impl(&state).await?;
        }
    }
    refresh_tray(&app).await;
    Ok(settings)
}

#[tauri::command]
async fn set_enabled(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
    enabled: bool,
) -> Result<ProcessStatus, String> {
    let result = if enabled {
        bring_engine_up(&state).await
    } else {
        bring_engine_down(&state).await
    };
    refresh_tray(&app).await;
    result
}

#[tauri::command]
async fn engine_start(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
) -> Result<ProcessStatus, String> {
    let result = bring_engine_up(&state).await;
    refresh_tray(&app).await;
    result
}

#[tauri::command]
async fn engine_stop(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
) -> Result<ProcessStatus, String> {
    let result = bring_engine_down(&state).await;
    refresh_tray(&app).await;
    result
}

#[tauri::command]
async fn set_active_strategy(
    state: State<'_, CoreState>,
    engine: Engine,
    id: String,
) -> Result<Settings, String> {
    let mut settings = settings_of(&state);
    let strategy = repository::by_id(&state.paths, &id)
        .ok_or_else(|| format!("Стратегия {id} не найдена"))?;
    if strategy.engine != engine {
        return Err("Движок стратегии не совпадает с выбранным".into());
    }
    settings.set_active(strategy.engine, &id);
    settings.engine = strategy.engine;
    persist_settings(&state, &settings)?;
    if settings.enabled {
        let _ = bring_engine_up(&state).await;
    }
    Ok(settings)
}

#[tauri::command]
async fn set_strategy_option(
    state: State<'_, CoreState>,
    strategy_id: String,
    variable: String,
    index: i64,
) -> Result<Settings, String> {
    let mut settings = settings_of(&state);
    settings.set_option(&strategy_id, &variable, index);
    let is_active = repository::by_id(&state.paths, &strategy_id)
        .map(|s| settings.active_strategy(s.engine) == Some(&strategy_id))
        .unwrap_or(false);
    persist_settings(&state, &settings)?;
    if settings.enabled && is_active {
        let _ = bring_engine_up(&state).await;
    }
    Ok(settings)
}

#[tauri::command]
async fn set_strategy_flag(
    state: State<'_, CoreState>,
    strategy_id: String,
    key: String,
    value: bool,
) -> Result<Settings, String> {
    let mut settings = settings_of(&state);
    settings.set_flag(&strategy_id, &key, value);
    let is_active = repository::by_id(&state.paths, &strategy_id)
        .map(|s| settings.active_strategy(s.engine) == Some(&strategy_id))
        .unwrap_or(false);
    persist_settings(&state, &settings)?;
    if settings.enabled && is_active {
        let _ = bring_engine_up(&state).await;
    }
    Ok(settings)
}

#[tauri::command]
async fn catalog(engine: Engine, full: bool) -> Result<Vec<Strategy>, String> {
    Ok(builtin::catalog(engine, full))
}

#[tauri::command]
async fn install_component(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
    id: String,
    force: bool,
) -> Result<ComponentStatus, String> {
    let app_progress = app.clone();
    let progress = move |p: components::Progress| {
        let _ = app_progress.emit("component-progress", &p);
    };
    let engine_log = state.paths.logs().join("engine.log");
    let proxy_log = state.paths.logs().join("tgws.log");
    match id.as_str() {
        "zapret" => {
            let was_running = state.processes.engine_running().await;
            if was_running {
                state.processes.stop_engine(&engine_log).await;
            }
            let latest = components::latest_zapret_version().await?;
            components::install_zapret(&state.paths, &latest, &progress, force).await?;
            if was_running && settings_of(&state).enabled {
                let _ = bring_engine_up(&state).await;
            }
            app_log(&state, &format!("component zapret installed: {latest}"));
        }
        "tgws" => {
            let was_running = state.processes.proxy_status(&proxy_log).await.running;
            if was_running {
                state.processes.stop_proxy(&proxy_log).await;
            }
            let release = components::latest_tgws_release().await?;
            components::install_tgws(&state.paths, &release, &progress, force).await?;
            if was_running {
                let _ = start_proxy_impl(&state).await;
            }
            app_log(&state, &format!("component tgws installed: {}", release.tag));
        }
        other => return Err(format!("Неизвестный компонент {other}")),
    }
    let _ = app.emit("components-changed", ());
    components::status(&state.paths)
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| "Компонент не найден".to_string())
}

#[tauri::command]
async fn check_updates(state: State<'_, CoreState>) -> Result<Vec<ComponentStatus>, String> {
    let list = components::check_updates(&state.paths).await;
    let mut settings = settings_of(&state);
    settings.last_update_check = manka_core::util::now_ms();
    let _ = persist_settings(&state, &settings);
    Ok(list)
}

#[tauri::command]
async fn rollback_component(
    state: State<'_, CoreState>,
    id: String,
) -> Result<ComponentStatus, String> {
    if id != "zapret" {
        return Err("Откат поддерживается только для Zapret".into());
    }
    let version = components::rollback_zapret(&state.paths)?;
    let _version = version;
    components::status(&state.paths)
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| "Компонент не найден".to_string())
}

#[tauri::command]
async fn tgws_start(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
) -> Result<ProcessStatus, String> {
    let result = start_proxy_impl(&state).await;
    refresh_tray(&app).await;
    result
}

async fn start_proxy_impl(state: &CoreState) -> Result<ProcessStatus, String> {
    let mut settings = settings_of(state);
    let exe = state
        .paths
        .tgws_exe()
        .ok_or_else(|| "TG WS Proxy не установлен".to_string())?;
    let log = state.paths.logs().join("tgws.log");
    // our own proxy may be running on the same port: stop it before the check
    state.processes.stop_proxy(&log).await;
    if let Some((pid, name)) = manka_core::util::port_owner(settings.tgws_port) {
        return Err(format!(
            "Порт {} занят: {name} (PID {pid}). Остановите его на экране Telegram или смените порт.",
            settings.tgws_port
        ));
    }
    let args = args::tgws_args(&settings);
    state
        .processes
        .start_proxy(&exe, &args, exe.parent(), &log)
        .await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let status = state.processes.proxy_status(&log).await;
    if !status.running {
        let tail = manka_core::util::tail_lines(&log, 10).join(" | ");
        return Err(format!("Прокси завершился: {tail}"));
    }
    settings.tgws_enabled = true;
    persist_settings(state, &settings)?;
    app_log(state, "tgws started");
    Ok(status)
}

#[tauri::command]
async fn tgws_stop(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
) -> Result<ProcessStatus, String> {
    let result = stop_proxy_impl(&state).await;
    refresh_tray(&app).await;
    result
}

async fn stop_proxy_impl(state: &CoreState) -> Result<ProcessStatus, String> {
    let log = state.paths.logs().join("tgws.log");
    state.processes.stop_proxy(&log).await;
    let mut settings = settings_of(state);
    settings.tgws_enabled = false;
    persist_settings(state, &settings)?;
    app_log(state, "tgws stopped");
    Ok(ProcessStatus::default())
}

#[derive(Serialize)]
struct TgwsCheck {
    ok: bool,
    output: String,
}

#[tauri::command]
async fn tgws_check(state: State<'_, CoreState>) -> Result<TgwsCheck, String> {
    let settings = settings_of(&state);
    let exe = state
        .paths
        .tgws_exe()
        .ok_or_else(|| "TG WS Proxy не установлен".to_string())?;
    let mut args: Vec<String> = args::tgws_args(&settings)
        .into_iter()
        .filter(|a| a != "-q" && a != "--quiet")
        .collect();
    args.push("--check".into());
    let output = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new(&exe)
            .args(&args)
            .current_dir(exe.parent().unwrap_or(std::path::Path::new(".")))
            .output(),
    )
    .await
    .map_err(|_| "Проверка не завершилась за 60 секунд".to_string())?
    .map_err(|e| e.to_string())?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let ok = output.status.success() && (text.contains("[OK") || text.trim().is_empty());
    Ok(TgwsCheck { ok, output: text.trim().to_string() })
}

#[tauri::command]
async fn tgws_regenerate_secret(state: State<'_, CoreState>) -> Result<Settings, String> {
    let mut settings = settings_of(&state);
    settings.tgws_secret = Settings::default().tgws_secret;
    persist_settings(&state, &settings)?;
    if settings.tgws_enabled {
        let _ = tgws_restart_impl(&state).await;
    }
    Ok(settings)
}

async fn tgws_restart_impl(state: &CoreState) -> Result<(), String> {
    let settings = settings_of(state);
    let exe = state.paths.tgws_exe().ok_or("TG WS Proxy не установлен")?;
    let log = state.paths.logs().join("tgws.log");
    let args = args::tgws_args(&settings);
    state
        .processes
        .start_proxy(&exe, &args, exe.parent(), &log)
        .await?;
    Ok(())
}

#[tauri::command]
fn tgws_link(state: State<'_, CoreState>) -> Result<String, String> {
    Ok(args::tgws_link(&settings_of(&state)))
}

#[tauri::command]
async fn store_refresh(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
) -> Result<usize, String> {
    let app2 = app.clone();
    let progress = move |p: components::Progress| {
        let _ = app2.emit("store-progress", &p);
    };
    store::refresh_index(&state.paths, &progress).await?;
    let _ = app.emit("store-changed", ());
    Ok(store::items(&state.paths).len())
}

#[tauri::command]
async fn store_items(state: State<'_, CoreState>) -> Result<Vec<StoreItem>, String> {
    Ok(store::items_with_installed(&state.paths))
}

#[tauri::command]
async fn store_install(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
    store_id: String,
) -> Result<String, String> {
    let app2 = app.clone();
    let progress = move |p: components::Progress| {
        let _ = app2.emit("store-progress", &p);
    };
    let version = store::install_item(&state.paths, &store_id, &progress).await?;
    app_log(&state, &format!("kit {store_id} installed {version}"));
    let _ = app.emit("store-changed", ());
    Ok(version)
}

#[tauri::command]
async fn store_remove(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
    store_id: String,
) -> Result<(), String> {
    store::remove_kit(&state.paths, &store_id)?;
    let _ = app.emit("store-changed", ());
    Ok(())
}

#[tauri::command]
async fn probe_targets(
    state: State<'_, CoreState>,
    targets: Vec<String>,
    requests: u32,
    timeout_sec: u32,
) -> Result<Vec<SiteResult>, String> {
    let targets = if targets.is_empty() {
        settings_of(&state).all_targets()
    } else {
        targets
    };
    Ok(manka_core::probe::check_sites(&targets, requests.max(1), timeout_sec.max(2), 8).await)
}

#[tauri::command]
async fn auto_start(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
    request: AutoRequest,
) -> Result<AutoState, String> {
    let _guard = state
        .auto_lock
        .try_lock()
        .map_err(|_| "Автоподбор уже выполняется".to_string())?;
    if !manka_core::util::is_elevated() {
        return Err("Автоподбор требует прав администратора: перезапустите Manka от имени администратора.".into());
    }
    state.cancel.store(false, Ordering::Relaxed);
    let settings = settings_of(&state);
    let mut strategies = repository::all(&state.paths);
    for engine in &request.engines {
        strategies.extend(builtin::catalog(*engine, request.full));
    }
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
    if settings.enabled {
        let _ = bring_engine_up(&state).await;
    }
    refresh_tray(&app).await;
    let _ = app.emit("auto-finished", &result);
    Ok(result)
}

#[tauri::command]
fn auto_cancel(state: State<'_, CoreState>) {
    state.cancel.store(true, Ordering::Relaxed);
    app_log(&state, "autoselect cancel requested");
}

#[derive(serde::Deserialize)]
struct ApplyCandidate {
    engine: Engine,
    name: String,
    template: String,
    percent: u32,
}

#[tauri::command]
async fn auto_apply(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
    candidate: ApplyCandidate,
) -> Result<Strategy, String> {
    let saved = repository::save_auto(
        &state.paths,
        &candidate.name,
        candidate.engine,
        &candidate.template,
        candidate.percent,
    )?;
    let mut settings = settings_of(&state);
    settings.set_active(candidate.engine, &saved.id);
    settings.engine = candidate.engine;
    settings.enabled = true;
    settings.last_check = manka_core::util::now_ms();
    settings.last_check_rate = candidate.percent as i64;
    persist_settings(&state, &settings)?;
    bring_engine_up(&state).await?;
    refresh_tray(&app).await;
    Ok(saved)
}

#[tauri::command]
fn get_history(state: State<'_, CoreState>) -> Vec<manka_core::history::HistoryRun> {
    manka_core::history::load(&state.paths)
}

fn log_path(state: &CoreState, name: &str) -> std::path::PathBuf {
    match name {
        "engine" => state.paths.logs().join("engine.log"),
        "tgws" | "proxy" => state.paths.logs().join("tgws.log"),
        _ => state.paths.logs().join("app.log"),
    }
}

#[derive(Serialize)]
struct LogInfoDto {
    exists: bool,
    size: u64,
    modified: i64,
}

#[tauri::command]
fn log_info(state: State<'_, CoreState>, name: String) -> LogInfoDto {
    let path = log_path(&state, &name);
    match std::fs::metadata(&path) {
        Ok(meta) => {
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            LogInfoDto {
                exists: true,
                size: meta.len(),
                modified,
            }
        }
        Err(_) => LogInfoDto {
            exists: false,
            size: 0,
            modified: 0,
        },
    }
}

#[tauri::command]
fn read_log(state: State<'_, CoreState>, name: String, lines: usize) -> Vec<String> {
    manka_core::util::tail_lines(&log_path(&state, &name), lines.clamp(1, 2000))
}

#[tauri::command]
fn clear_log(state: State<'_, CoreState>, name: String) -> Result<(), String> {
    std::fs::write(log_path(&state, &name), b"").map_err(|e| e.to_string())
}

#[tauri::command]
fn is_elevated() -> bool {
    manka_core::util::is_elevated()
}

#[tauri::command]
fn relaunch_elevated() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy().replace('\'', "''");
    // The single-instance lock is held by this process, so the elevated copy
    // must start after we exit. PowerShell waits 2s, shows the UAC prompt and
    // launches the app; this process then terminates to release the lock.
    let script = format!(
        "Start-Sleep -Seconds 2; Start-Process -FilePath '{exe}' -Verb RunAs"
    );
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    std::process::exit(0);
}

#[tauri::command]
fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

fn github_repo(repo: &str) -> Option<String> {
    let mut repo = repo.trim();
    for prefix in ["https://github.com/", "http://github.com/", "github.com/"] {
        if let Some(rest) = repo.strip_prefix(prefix) {
            repo = rest;
            break;
        }
    }
    let repo = repo.trim_end_matches('/');
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    let mut parts = repo.split('/');
    let owner = parts.next()?;
    let name = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let valid = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    if valid(owner) && valid(name) {
        Some(format!("{owner}/{name}"))
    } else {
        None
    }
}

fn build_updater(
    app: &tauri::AppHandle,
    repo: &str,
) -> Result<tauri_plugin_updater::Updater, String> {
    let repo = github_repo(repo)
        .ok_or("Укажите GitHub-репозиторий в формате owner/repo (например, ivalu/manka)")?;
    let endpoint = format!("https://github.com/{repo}/releases/latest/download/latest.json");
    let url = url::Url::parse(&endpoint).map_err(|e| e.to_string())?;
    app.updater_builder()
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct AppUpdateInfo {
    current_version: String,
    version: String,
    notes: Option<String>,
    date: Option<String>,
    download_url: String,
    releases_url: String,
}

#[tauri::command]
async fn app_update_check(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
) -> Result<Option<AppUpdateInfo>, String> {
    let mut settings = settings_of(&state);
    let updater = build_updater(&app, &settings.update_repo)?;
    let found = updater.check().await.map_err(|e| e.to_string())?;
    settings.last_app_update_check = manka_core::util::now_ms();
    persist_settings(&state, &settings)?;
    let repo = github_repo(&settings.update_repo).unwrap_or_default();
    Ok(found.map(|u| AppUpdateInfo {
        current_version: u.current_version.clone(),
        version: u.version.clone(),
        notes: u.body.clone(),
        date: u.date.map(|d| d.to_string()),
        download_url: u.download_url.to_string(),
        releases_url: format!("https://github.com/{repo}/releases"),
    }))
}

#[tauri::command]
async fn app_update_install(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
) -> Result<(), String> {
    let settings = settings_of(&state);
    let updater = build_updater(&app, &settings.update_repo)?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Обновлений не найдено")?;
    let handle = app.clone();
    let mut downloaded: u64 = 0;
    let _ = app.emit("app-update-started", ());
    update
        .download_and_install(
            move |chunk: usize, total: Option<u64>| {
                downloaded += chunk as u64;
                let _ = handle.emit(
                    "app-update-progress",
                    serde_json::json!({ "downloaded": downloaded, "total": total }),
                );
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_data_dir(state: State<'_, CoreState>) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(state.paths.root())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn open_logs_dir(state: State<'_, CoreState>) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(state.paths.logs())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart(state: State<'_, CoreState>, enabled: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy().to_string();
    if enabled {
        // A scheduled task with the highest run level starts Manka elevated at
        // logon without a UAC prompt (the task is created by the elevated app).
        let task = std::process::Command::new("schtasks")
            .args([
                "/Create", "/SC", "ONLOGON", "/TN", "Manka", "/TR", &format!("\"{exe}\""),
                "/RL", "HIGHEST", "/F",
            ])
            .status();
        let task_ok = task.map(|s| s.success()).unwrap_or(false);
        if !task_ok {
            let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
            let reg = std::process::Command::new("reg")
                .args(["add", key, "/v", "Manka", "/t", "REG_SZ", "/d", &exe, "/f"])
                .status()
                .map_err(|e| e.to_string())?;
            if !reg.success() {
                return Err("Не удалось создать автозапуск".into());
            }
        }
    } else {
        let _ = std::process::Command::new("schtasks")
            .args(["/Delete", "/TN", "Manka", "/F"])
            .status();
        let _ = std::process::Command::new("reg")
            .args(["delete", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "Manka", "/f"])
            .status();
    }
    let mut settings = settings_of(&state);
    settings.autostart = enabled;
    persist_settings(&state, &settings)?;
    Ok(())
}

#[tauri::command]
fn delete_strategy(state: State<'_, CoreState>, id: String) -> Result<(), String> {
    repository::delete_user(&state.paths, &id)
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = Paths::detect();
    let _ = paths.ensure();
    let _ = manka_core::settings::ensure_exclude_list(&paths);
    if manka_core::util::is_elevated() {
        let killed = manka_core::util::kill_stale_components(&paths.components());
        if killed > 0 {
            manka_core::util::append_log(
                &paths.logs().join("app.log"),
                &format!("cleaned up {killed} stale component process(es)"),
            );
        }
    }
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
            build_tray(app)?;
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Manka");
}


