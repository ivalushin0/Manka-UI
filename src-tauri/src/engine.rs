use std::time::Duration;

use manka_core::model::ProcessStatus;
use manka_core::{args, repository};

use crate::state::{app_log, persist_settings, settings_of, CoreState};

pub(crate) async fn bring_engine_up(state: &CoreState) -> Result<ProcessStatus, String> {
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
        &format!(
            "engine started: {} ({})",
            strategy.name, rendered.command_line
        ),
    );
    Ok(status)
}

pub(crate) async fn bring_engine_down(state: &CoreState) -> Result<ProcessStatus, String> {
    let log = state.paths.logs().join("engine.log");
    state.processes.stop_engine(&log).await;
    let mut settings = settings_of(state);
    settings.enabled = false;
    persist_settings(state, &settings)?;
    app_log(state, "engine stopped");
    Ok(ProcessStatus::default())
}

pub(crate) async fn stop_everything(state: &CoreState) {
    let engine_log = state.paths.logs().join("engine.log");
    let proxy_log = state.paths.logs().join("tgws.log");
    state.processes.stop_all(&engine_log, &proxy_log).await;
    let mut settings = settings_of(state);
    settings.enabled = false;
    settings.tgws_enabled = false;
    let _ = persist_settings(state, &settings);
}

pub(crate) async fn start_proxy_impl(state: &CoreState) -> Result<ProcessStatus, String> {
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

pub(crate) async fn stop_proxy_impl(state: &CoreState) -> Result<ProcessStatus, String> {
    let log = state.paths.logs().join("tgws.log");
    state.processes.stop_proxy(&log).await;
    let mut settings = settings_of(state);
    settings.tgws_enabled = false;
    persist_settings(state, &settings)?;
    app_log(state, "tgws stopped");
    Ok(ProcessStatus::default())
}

pub(crate) async fn tgws_restart_impl(state: &CoreState) -> Result<(), String> {
    // start_proxy_impl stops our own proxy, verifies the port is free and
    // marks tgws_enabled — everything a restart needs
    start_proxy_impl(state).await.map(|_| ())
}
