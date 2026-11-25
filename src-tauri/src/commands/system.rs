use serde::Serialize;
use tauri::{Emitter, State};
use tauri_plugin_opener::OpenerExt;

use crate::state::{app_log, persist_settings, settings_of, CoreState};

fn log_path(state: &CoreState, name: &str) -> std::path::PathBuf {
    match name {
        "engine" => state.paths.logs().join("engine.log"),
        "tgws" | "proxy" => state.paths.logs().join("tgws.log"),
        _ => state.paths.logs().join("app.log"),
    }
}

#[derive(Serialize)]
pub struct LogInfoDto {
    pub exists: bool,
    pub size: u64,
    pub modified: i64,
}

#[tauri::command]
pub fn log_info(state: State<'_, CoreState>, name: String) -> LogInfoDto {
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
pub fn read_log(state: State<'_, CoreState>, name: String, lines: usize) -> Vec<String> {
    manka_core::util::tail_lines(&log_path(&state, &name), lines.clamp(1, 2000))
}

#[tauri::command]
pub fn clear_log(state: State<'_, CoreState>, name: String) -> Result<(), String> {
    std::fs::write(log_path(&state, &name), b"").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn is_elevated() -> bool {
    manka_core::util::is_elevated()
}

#[tauri::command]
pub fn relaunch_elevated() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy().replace('\'', "''");
    // The single-instance lock is held by this process, so the elevated copy
    // must start after we exit. PowerShell waits 2s, shows the UAC prompt and
    // launches the app; this process then terminates to release the lock.
    let script = format!("Start-Sleep -Seconds 2; Start-Process -FilePath '{exe}' -Verb RunAs");
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
pub fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    // The URL can come from remote markdown (kit descriptions, release notes),
    // so only allow the two schemes we ever intend to open.
    let lower = url.to_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("tg://")) {
        return Err(format!("Запрещённая схема URL: {url}"));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_data_dir(state: State<'_, CoreState>) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(state.paths.root())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_logs_dir(state: State<'_, CoreState>) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(state.paths.logs())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_autostart(state: State<'_, CoreState>, enabled: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy().to_string();
    if enabled {
        // A scheduled task with the highest run level starts Manka elevated at
        // logon without a UAC prompt (the task is created by the elevated app).
        let task = std::process::Command::new("schtasks")
            .args([
                "/Create",
                "/SC",
                "ONLOGON",
                "/TN",
                "Manka",
                "/TR",
                &format!("\"{exe}\""),
                "/RL",
                "HIGHEST",
                "/F",
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
            .args([
                "delete",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                "Manka",
                "/f",
            ])
            .status();
    }
    let mut settings = settings_of(&state);
    settings.autostart = enabled;
    persist_settings(&state, &settings)?;
    Ok(())
}

// ---------------------------------------------------------------- P2: lists / doctor / watchdog

#[tauri::command]
pub async fn lists_update_now(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
) -> Result<manka_core::lists::ListsReport, String> {
    let settings = settings_of(&state);
    let report = manka_core::lists::update(&state.paths, &settings).await;
    if report.sources_ok == 0 {
        return Err(if report.errors.is_empty() {
            "Нет источников списков".to_string()
        } else {
            report.errors.join("; ")
        });
    }
    let mut s = settings_of(&state);
    s.last_lists_update = report.updated_at;
    let _ = persist_settings(&state, &s);
    app_log(
        &state,
        &format!(
            "lists updated: {} domains, {} source(s) ok, {} failed",
            report.domains, report.sources_ok, report.sources_failed
        ),
    );
    let _ = app.emit("lists-updated", &report);
    Ok(report)
}

#[tauri::command]
pub fn doctor_run(state: State<'_, CoreState>) -> manka_core::doctor::DoctorReport {
    manka_core::doctor::run(&state.paths, &settings_of(&state))
}

#[tauri::command]
pub fn doctor_export(state: State<'_, CoreState>) -> Result<String, String> {
    let path = manka_core::doctor::export(&state.paths, &settings_of(&state))?;
    app_log(&state, &format!("diagnostics exported: {}", path.display()));
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn watchdog_check_now(
    app: tauri::AppHandle,
    state: State<'_, CoreState>,
) -> Result<manka_core::watchdog::WatchdogReport, String> {
    let settings = settings_of(&state);
    let report = manka_core::watchdog::check(&settings).await;
    let mut s = settings_of(&state);
    s.last_watchdog_check = report.checked_at;
    let _ = persist_settings(&state, &s);
    app_log(
        &state,
        &format!(
            "watchdog (manual): healthy={} {}",
            report.healthy, report.detail
        ),
    );
    let _ = app.emit("watchdog", &report);
    Ok(report)
}
