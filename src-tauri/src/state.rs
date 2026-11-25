use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use manka_core::paths::Paths;
use manka_core::runner::ProcessManager;
use manka_core::settings::Settings;

pub struct CoreState {
    pub paths: Paths,
    pub settings: Mutex<Settings>,
    pub processes: ProcessManager,
    pub cancel: AtomicBool,
    pub auto_lock: tokio::sync::Mutex<()>,
    /// Last picture painted into the tray, so the polling watcher only redraws
    /// the icon/menu when something actually changed.
    pub(crate) tray: Mutex<Option<TraySnapshot>>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct TraySnapshot {
    pub(crate) healthy: bool,
    pub(crate) tooltip: String,
    pub(crate) engine_text: String,
    pub(crate) proxy_text: String,
    pub(crate) engine_checked: bool,
    pub(crate) proxy_checked: bool,
}

pub(crate) fn settings_of(state: &CoreState) -> Settings {
    state.settings.lock().map(|s| s.clone()).unwrap_or_default()
}

pub(crate) fn persist_settings(state: &CoreState, settings: &Settings) -> Result<(), String> {
    settings.save(&state.paths).map_err(|e| e.to_string())?;
    if let Ok(mut slot) = state.settings.lock() {
        *slot = settings.clone();
    }
    Ok(())
}

pub(crate) fn app_log(state: &CoreState, line: &str) {
    manka_core::util::append_log(&state.paths.logs().join("app.log"), line);
}
