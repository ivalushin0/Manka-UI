//! Child process management: winws/winws2 and tg-ws-proxy-rs.

use std::fs::OpenOptions;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::process::{Child, Command};

use crate::model::ProcessStatus;

pub struct Running {
    child: Child,
    started: Instant,
    pub exe: std::path::PathBuf,
    pub args: Vec<String>,
}

impl Running {
    pub fn uptime_sec(&self) -> u64 {
        self.started.elapsed().as_secs()
    }
}

#[derive(Default)]
pub struct ProcessManager {
    engine: tokio::sync::Mutex<Option<Running>>,
    proxy: tokio::sync::Mutex<Option<Running>>,
}

fn create_no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

fn spawn_with_log(
    exe: &Path,
    args: &[String],
    workdir: Option<&Path>,
    log: &Path,
) -> Result<Child, String> {
    if let Some(parent) = log.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let out = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .map_err(|e| format!("log {}: {e}", log.display()))?;
    let err = out.try_clone().map_err(|e| e.to_string())?;

    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));
    if let Some(dir) = workdir {
        cmd.current_dir(dir);
    }
    create_no_window(&mut cmd);
    cmd.spawn()
        .map_err(|e| format!("{}: {e}", exe.display()))
}

async fn stop_locked(slot: &mut Option<Running>, log: &Path) {
    if let Some(mut run) = slot.take() {
        let pid = run.child.id().unwrap_or(0);
        let uptime = run.uptime_sec();
        let _ = run.child.start_kill();
        let waited = tokio::time::timeout(Duration::from_secs(4), run.child.wait()).await;
        if waited.is_err() {
            let _ = run.child.kill().await;
        }
        crate::util::append_log(log, &format!("process stopped (pid {pid}, {uptime}s)"));
    }
}

fn status_of(slot: &mut Option<Running>, log: &Path) -> ProcessStatus {
    if let Some(run) = slot.as_mut() {
        match run.child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(-1);
                crate::util::append_log(log, &format!("process exited with {code}"));
                let detail = format!("процесс завершился с кодом {code}");
                *slot = None;
                ProcessStatus {
                    running: false,
                    pid: None,
                    uptime_sec: 0,
                    engine: None,
                    detail: Some(detail),
                }
            }
            Ok(None) => ProcessStatus {
                running: true,
                pid: run.child.id(),
                uptime_sec: run.uptime_sec(),
                engine: Some(run.exe.to_string_lossy().to_string()),
                detail: None,
            },
            Err(e) => ProcessStatus {
                running: false,
                pid: None,
                uptime_sec: 0,
                engine: None,
                detail: Some(e.to_string()),
            },
        }
    } else {
        ProcessStatus::default()
    }
}

impl ProcessManager {
    pub async fn start_engine(
        &self,
        exe: &Path,
        args: &[String],
        workdir: Option<&Path>,
        log: &Path,
    ) -> Result<u32, String> {
        let mut slot = self.engine.lock().await;
        stop_locked(&mut slot, log).await;
        if !exe.exists() {
            return Err(format!("Не найден {}", exe.display()));
        }
        let child = spawn_with_log(exe, args, workdir, log)?;
        let pid = child.id().unwrap_or(0);
        crate::util::append_log(
            log,
            &format!(
                "started pid={pid} exe={} args={}",
                exe.display(),
                crate::args::join(args)
            ),
        );
        *slot = Some(Running {
            child,
            started: Instant::now(),
            exe: exe.to_path_buf(),
            args: args.to_vec(),
        });
        Ok(pid)
    }

    pub async fn stop_engine(&self, log: &Path) {
        let mut slot = self.engine.lock().await;
        stop_locked(&mut slot, log).await;
    }

    pub async fn engine_status(&self, log: &Path) -> ProcessStatus {
        let mut slot = self.engine.lock().await;
        status_of(&mut slot, log)
    }

    pub async fn start_proxy(
        &self,
        exe: &Path,
        args: &[String],
        workdir: Option<&Path>,
        log: &Path,
    ) -> Result<u32, String> {
        let mut slot = self.proxy.lock().await;
        stop_locked(&mut slot, log).await;
        let child = spawn_with_log(exe, args, workdir, log)?;
        let pid = child.id().unwrap_or(0);
        crate::util::append_log(
            log,
            &format!(
                "started pid={pid} exe={} args={}",
                exe.display(),
                crate::args::join(args)
            ),
        );
        *slot = Some(Running {
            child,
            started: Instant::now(),
            exe: exe.to_path_buf(),
            args: args.to_vec(),
        });
        Ok(pid)
    }

    pub async fn stop_proxy(&self, log: &Path) {
        let mut slot = self.proxy.lock().await;
        stop_locked(&mut slot, log).await;
    }

    pub async fn proxy_status(&self, log: &Path) -> ProcessStatus {
        let mut slot = self.proxy.lock().await;
        status_of(&mut slot, log)
    }

    pub async fn stop_all(&self, engine_log: &Path, proxy_log: &Path) {
        self.stop_engine(engine_log).await;
        self.stop_proxy(proxy_log).await;
    }

    pub async fn engine_running(&self) -> bool {
        let mut slot = self.engine.lock().await;
        if let Some(run) = slot.as_mut() {
            matches!(run.child.try_wait(), Ok(None))
        } else {
            false
        }
    }
}
