use std::time::Duration;

use manka_core::args;
use manka_core::model::ProcessStatus;
use manka_core::settings::Settings;
use serde::Serialize;
use tauri::State;

use crate::engine::{start_proxy_impl, stop_proxy_impl, tgws_restart_impl};
use crate::state::{persist_settings, settings_of, CoreState};
use crate::tray::refresh_tray;

#[tauri::command]
pub async fn tgws_start(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
) -> Result<ProcessStatus, String> {
    let result = start_proxy_impl(&state).await;
    refresh_tray(&app).await;
    result
}

#[tauri::command]
pub async fn tgws_stop(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
) -> Result<ProcessStatus, String> {
    let result = stop_proxy_impl(&state).await;
    refresh_tray(&app).await;
    result
}

#[derive(Serialize)]
pub struct TgwsCheck {
    pub ok: bool,
    pub output: String,
}

#[tauri::command]
pub async fn tgws_check(state: State<'_, CoreState>) -> Result<TgwsCheck, String> {
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
    Ok(TgwsCheck {
        ok,
        output: text.trim().to_string(),
    })
}

#[tauri::command]
pub async fn tgws_regenerate_secret(state: State<'_, CoreState>) -> Result<Settings, String> {
    let mut settings = settings_of(&state);
    settings.tgws_secret = Settings::default().tgws_secret;
    persist_settings(&state, &settings)?;
    if settings.tgws_enabled {
        let _ = tgws_restart_impl(&state).await;
    }
    Ok(settings)
}

#[tauri::command]
pub fn tgws_link(state: State<'_, CoreState>) -> Result<String, String> {
    Ok(args::tgws_link(&settings_of(&state)))
}
