use manka_core::model::StoreItem;
use manka_core::{components, store};
use tauri::{Emitter, State};

use crate::state::{app_log, CoreState};

#[tauri::command]
pub async fn store_refresh(
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
pub async fn store_items(state: State<'_, CoreState>) -> Result<Vec<StoreItem>, String> {
    Ok(store::items_with_installed(&state.paths))
}

#[tauri::command]
pub async fn store_install(
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
pub async fn store_remove(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
    store_id: String,
) -> Result<(), String> {
    store::remove_kit(&state.paths, &store_id)?;
    let _ = app.emit("store-changed", ());
    Ok(())
}
