use manka_core::model::{ComponentStatus, Engine, ProcessStatus, Strategy};
use manka_core::settings::Settings;
use manka_core::{builtin, components, repository, store};
use serde::Serialize;
use tauri::{Emitter, State};

use crate::engine::{bring_engine_down, bring_engine_up, start_proxy_impl};
use crate::state::{app_log, persist_settings, settings_of, CoreState};
use crate::tray::refresh_tray;

#[derive(Serialize)]
pub struct TargetGroupDto {
    pub id: String,
    pub title: String,
    pub urls: Vec<String>,
}

#[derive(Serialize)]
pub struct KitDto {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(Serialize)]
pub struct AppDto {
    pub version: String,
    pub data_dir: String,
    pub elevated: bool,
    pub settings: Settings,
    pub strategies: Vec<Strategy>,
    pub engine: ProcessStatus,
    pub proxy: ProcessStatus,
    pub components: Vec<ComponentStatus>,
    pub target_groups: Vec<TargetGroupDto>,
    pub kits: Vec<KitDto>,
    pub history: Vec<manka_core::history::HistoryRun>,
}

#[tauri::command]
pub async fn get_state(state: State<'_, CoreState>) -> Result<AppDto, String> {
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
    })
}

#[derive(Serialize)]
pub struct StatusDto {
    pub engine: ProcessStatus,
    pub proxy: ProcessStatus,
}

#[tauri::command]
pub async fn get_status(state: State<'_, CoreState>) -> Result<StatusDto, String> {
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
pub struct PortOwnerDto {
    pub pid: u32,
    pub name: String,
}

#[tauri::command]
pub fn port_owner(port: u16) -> Option<PortOwnerDto> {
    manka_core::util::port_owner(port).map(|(pid, name)| PortOwnerDto { pid, name })
}

#[tauri::command]
pub fn stop_port_owner(port: u16) -> Result<String, String> {
    manka_core::util::stop_port_owner(port)
}

#[tauri::command]
pub async fn save_settings(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
    mut settings: Settings,
) -> Result<Settings, String> {
    if settings.tgws_secret.trim().is_empty() {
        settings.tgws_secret = Settings::default().tgws_secret;
    }
    settings.tgws_port = settings.tgws_port.clamp(1024, 65535);
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
pub async fn set_enabled(
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
pub async fn engine_start(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
) -> Result<ProcessStatus, String> {
    let result = bring_engine_up(&state).await;
    refresh_tray(&app).await;
    result
}

#[tauri::command]
pub async fn engine_stop(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
) -> Result<ProcessStatus, String> {
    let result = bring_engine_down(&state).await;
    refresh_tray(&app).await;
    result
}

#[tauri::command]
pub async fn set_active_strategy(
    state: State<'_, CoreState>,
    engine: Engine,
    id: String,
) -> Result<Settings, String> {
    let mut settings = settings_of(&state);
    let strategy =
        repository::by_id(&state.paths, &id).ok_or_else(|| format!("Стратегия {id} не найдена"))?;
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
pub async fn set_strategy_option(
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
pub async fn set_strategy_flag(
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
pub async fn catalog(engine: Engine, full: bool) -> Result<Vec<Strategy>, String> {
    Ok(builtin::catalog(engine, full))
}

#[tauri::command]
pub async fn install_component(
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
            let commit = components::latest_zapret_commit().await?;
            let version =
                components::install_zapret(&state.paths, &commit, &progress, force).await?;
            if was_running && settings_of(&state).enabled {
                let _ = bring_engine_up(&state).await;
            }
            app_log(&state, &format!("component zapret installed: {version}"));
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
            app_log(
                &state,
                &format!("component tgws installed: {}", release.tag),
            );
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
pub async fn check_updates(state: State<'_, CoreState>) -> Result<Vec<ComponentStatus>, String> {
    let list = components::check_updates(&state.paths).await;
    let mut settings = settings_of(&state);
    settings.last_update_check = manka_core::util::now_ms();
    let _ = persist_settings(&state, &settings);
    Ok(list)
}

#[tauri::command]
pub async fn rollback_component(
    state: State<'_, CoreState>,
    id: String,
) -> Result<ComponentStatus, String> {
    if !matches!(id.as_str(), "zapret" | "tgws") {
        return Err(format!("Неизвестный компонент {id}"));
    }
    match id.as_str() {
        "zapret" => {
            components::rollback_zapret(&state.paths)?;
        }
        _ => {
            components::rollback_tgws(&state.paths)?;
        }
    }
    if id == "zapret" {
        // the engine picks up the restored version on next start
        if settings_of(&state).enabled {
            let _ = bring_engine_up(&state).await;
        }
    } else if settings_of(&state).tgws_enabled {
        let _ = start_proxy_impl(&state).await;
    }
    components::status(&state.paths)
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| "Компонент не найден".to_string())
}

#[tauri::command]
pub fn delete_strategy(state: State<'_, CoreState>, id: String) -> Result<(), String> {
    repository::delete_user(&state.paths, &id)
}
