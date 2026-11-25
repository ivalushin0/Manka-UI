use tauri::{Emitter, State};

use crate::state::{persist_settings, settings_of, CoreState};
use crate::updater::{build_updater, github_repo, AppUpdateInfo};

#[tauri::command]
pub async fn app_update_check(
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
pub async fn app_update_install(
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
