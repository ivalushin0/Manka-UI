use std::sync::atomic::Ordering;

use manka_core::autoselect::{self, AutoRequest, AutoState};
use manka_core::model::{Engine, SiteResult, Strategy};
use manka_core::paths::Paths;
use manka_core::{builtin, repository};
use tauri::{Emitter, State};

use crate::engine::bring_engine_up;
use crate::state::{app_log, persist_settings, settings_of, CoreState};
use crate::tray::refresh_tray;

#[tauri::command]
pub async fn probe_targets(
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
pub async fn auto_start(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
    request: AutoRequest,
) -> Result<AutoState, String> {
    let _guard = state
        .auto_lock
        .try_lock()
        .map_err(|_| "Автоподбор уже выполняется".to_string())?;
    if !manka_core::util::is_elevated() {
        return Err(
            "Автоподбор требует прав администратора: перезапустите Manka от имени администратора."
                .into(),
        );
    }
    state.cancel.store(false, Ordering::Relaxed);
    let settings = settings_of(&state);
    // candidates come from the repository (builtin+store+user) plus the
    // machine catalogue that `autoselect::candidates` adds per engine
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
    if settings.enabled {
        let _ = bring_engine_up(&state).await;
    }
    refresh_tray(&app).await;
    let _ = app.emit("auto-finished", &result);
    Ok(result)
}

#[tauri::command]
pub fn auto_cancel(state: State<'_, CoreState>) {
    state.cancel.store(true, Ordering::Relaxed);
    app_log(&state, "autoselect cancel requested");
}

#[derive(serde::Deserialize)]
pub struct ApplyCandidate {
    pub engine: Engine,
    pub name: String,
    pub template: String,
    pub percent: u32,
}

/// `auto_apply` runs winws with administrator rights, and winws can execute
/// arbitrary Lua (`--lua-init=@file`). So the template must come from a known
/// source: the built-in catalogue, installed strategies, or a recorded
/// autoselection history entry — never arbitrary IPC input.
fn template_allowed(paths: &Paths, engine: Engine, template: &str) -> bool {
    let t = template.trim();
    if t.is_empty() {
        return false;
    }
    builtin::catalog(engine, true)
        .iter()
        .any(|s| s.template.trim() == t)
        || repository::all(paths)
            .iter()
            .any(|s| s.engine == engine && s.template.trim() == t)
        || manka_core::history::load(paths).iter().any(|run| {
            run.results
                .iter()
                .any(|r| r.engine == engine.id() && r.template.trim() == t)
        })
}

#[tauri::command]
pub async fn auto_apply(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
    candidate: ApplyCandidate,
) -> Result<Strategy, String> {
    if !template_allowed(&state.paths, candidate.engine, &candidate.template) {
        return Err("Шаблон стратегии не найден в каталоге — применение отклонено".into());
    }
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
pub fn get_history(state: State<'_, CoreState>) -> Vec<manka_core::history::HistoryRun> {
    manka_core::history::load(&state.paths)
}
