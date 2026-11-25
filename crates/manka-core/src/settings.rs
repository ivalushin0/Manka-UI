use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::model::Engine;
use crate::paths::Paths;

fn default_secret() -> String {
    use rand::Rng;
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn default_port() -> u16 {
    crate::DEFAULT_TGWS_PORT
}

fn default_targets() -> Vec<String> {
    vec!["youtube".into(), "discord".into(), "general".into()]
}

fn default_requests() -> u32 {
    2
}

fn default_timeout() -> u32 {
    8
}

fn default_perfect() -> u32 {
    2
}

fn default_interval() -> u32 {
    6
}

fn default_language() -> String {
    "ru".into()
}

fn default_theme() -> String {
    "dark".into()
}

fn default_pool() -> u32 {
    4
}

fn default_buf_kb() -> u32 {
    256
}

fn default_scope() -> String {
    "list".into()
}

fn default_watchdog_interval() -> u32 {
    15
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    // ---- bypass
    pub enabled: bool,
    pub engine: Engine,
    /// engine id -> strategy id
    pub active: BTreeMap<String, String>,
    /// "<strategy id>::<variable>" -> selected index
    pub option_value: BTreeMap<String, i64>,
    /// "<strategy id>::<flag>" -> value
    pub flag_value: BTreeMap<String, bool>,
    pub fake_sni: String,
    pub block_quic: bool,
    pub autostart: bool,
    pub start_minimized: bool,
    /// "list" — обход только для доменов из manka-hostlist.txt (+ внешних
    /// списков); "all" — десинк применяется ко всему TLS/HTTP-трафику.
    pub bypass_scope: String,

    // ---- Telegram proxy
    pub tgws_enabled: bool,
    pub tgws_port: u16,
    pub tgws_secret: String,
    pub tgws_pool_size: u32,
    pub tgws_buf_kb: u32,
    pub tgws_cloudflare: bool,
    /// Comma/space/newline separated `DC:IP` entries.
    pub tgws_dc_ips: String,
    pub tgws_extra: String,

    // ---- diagnostics / autoselect
    pub targets: Vec<String>,
    pub custom_targets: String,
    pub autoselect_full: bool,
    pub autoselect_include_store: bool,
    pub autoselect_requests: u32,
    pub autoselect_timeout: u32,
    pub autoselect_stop_after_perfect: u32,
    pub autoselect_targets: Vec<String>,
    pub last_check: i64,
    pub last_check_rate: i64,

    // ---- watchdog (self-healing)
    pub watchdog_enabled: bool,
    pub watchdog_interval_min: u32,
    /// true — при деградации сразу запускать переподбор и применять лучшую
    /// стратегию; false — только уведомлять.
    pub watchdog_auto_fix: bool,
    pub last_watchdog_check: i64,

    // ---- external domain lists
    pub lists_auto_update: bool,
    pub lists_update_interval_hours: u32,
    /// Дополнительные https-источники списков доменов (по одному на строку).
    pub lists_sources: String,
    pub last_lists_update: i64,

    // ---- updates and UI
    pub update_interval_hours: u32,
    pub last_update_check: i64,
    /// GitHub repository for app self-updates, `owner/repo`.
    pub update_repo: String,
    pub auto_update_check: bool,
    pub app_update_interval_hours: u32,
    pub last_app_update_check: i64,
    pub debug_logs: bool,
    pub language: String,
    pub theme: String,
    pub close_to_tray: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            engine: Engine::Zapret2,
            active: BTreeMap::new(),
            option_value: BTreeMap::new(),
            flag_value: BTreeMap::new(),
            fake_sni: "www.google.com".into(),
            block_quic: true,
            autostart: false,
            start_minimized: true,
            bypass_scope: default_scope(),

            tgws_enabled: false,
            tgws_port: default_port(),
            tgws_secret: default_secret(),
            tgws_pool_size: default_pool(),
            tgws_buf_kb: default_buf_kb(),
            tgws_cloudflare: true,
            tgws_dc_ips: String::new(),
            tgws_extra: String::new(),

            targets: default_targets(),
            custom_targets: String::new(),
            autoselect_full: false,
            autoselect_include_store: true,
            autoselect_requests: default_requests(),
            autoselect_timeout: default_timeout(),
            autoselect_stop_after_perfect: default_perfect(),
            autoselect_targets: default_targets(),
            last_check: 0,
            last_check_rate: -1,

            watchdog_enabled: true,
            watchdog_interval_min: default_watchdog_interval(),
            watchdog_auto_fix: true,
            last_watchdog_check: 0,

            lists_auto_update: false,
            lists_update_interval_hours: 24,
            lists_sources: String::new(),
            last_lists_update: 0,

            update_interval_hours: default_interval(),
            last_update_check: 0,
            update_repo: "ivalushin0/Manka-UI".into(),
            auto_update_check: true,
            app_update_interval_hours: 24,
            last_app_update_check: 0,
            debug_logs: false,
            language: default_language(),
            theme: default_theme(),
            close_to_tray: true,
        }
    }
}

impl Settings {
    /// Load settings. A missing file yields defaults; a corrupt file is backed
    /// up next to the original and reported in the app log instead of being
    /// silently discarded.
    pub fn load(paths: &Paths) -> Self {
        let file = paths.settings_file();
        if file.exists() {
            match std::fs::read_to_string(&file) {
                Ok(text) => match serde_json::from_str::<Settings>(&text) {
                    Ok(settings) => return settings,
                    Err(e) => {
                        let backup = file.with_extension(format!(
                            "corrupt-{}.json",
                            chrono::Local::now().format("%Y%m%d-%H%M%S")
                        ));
                        let _ = std::fs::copy(&file, &backup);
                        crate::util::append_log(
                            &paths.logs().join("app.log"),
                            &format!(
                                "settings.json повреждён ({e}); копия сохранена в {}",
                                backup.display()
                            ),
                        );
                    }
                },
                Err(e) => {
                    crate::util::append_log(
                        &paths.logs().join("app.log"),
                        &format!("settings.json не читается: {e}"),
                    );
                }
            }
        }
        Self::default()
    }

    pub fn save(&self, paths: &Paths) -> std::io::Result<()> {
        crate::util::write_json(&paths.settings_file(), self)
    }

    pub fn active_strategy(&self, engine: Engine) -> Option<&String> {
        self.active.get(engine.id())
    }

    pub fn set_active(&mut self, engine: Engine, id: impl Into<String>) {
        self.active.insert(engine.id().to_string(), id.into());
    }

    pub fn option(&self, strategy_id: &str, variable: &str) -> Option<usize> {
        self.option_value
            .get(&format!("{strategy_id}::{variable}"))
            .copied()
            .filter(|v| *v >= 0)
            .map(|v| v as usize)
    }

    pub fn set_option(&mut self, strategy_id: &str, variable: &str, index: i64) {
        self.option_value
            .insert(format!("{strategy_id}::{variable}"), index);
    }

    pub fn flag(&self, strategy_id: &str, key: &str, default: bool) -> bool {
        self.flag_value
            .get(&format!("{strategy_id}::{key}"))
            .copied()
            .unwrap_or(default)
    }

    pub fn set_flag(&mut self, strategy_id: &str, key: &str, value: bool) {
        self.flag_value
            .insert(format!("{strategy_id}::{key}"), value);
    }

    /// Selected targets plus the user's own URLs.
    pub fn all_targets(&self) -> Vec<String> {
        let mut out = crate::group_urls(&self.targets);
        for line in self.custom_targets.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let url = if line.starts_with("http://") || line.starts_with("https://") {
                line.to_string()
            } else {
                format!("https://{line}/")
            };
            out.push(url);
        }
        out.sort();
        out.dedup();
        out
    }
}

/// Create `lists/manka-exclude.txt` once. Banks, government services, Russian
/// CDNs and AI services do not need bypass and can break when TLS is altered.
const DEFAULT_EXCLUDE: &str = include_str!("../assets/manka-exclude.txt");
/// Hosts that should be desynced. The rest of the traffic is left untouched.
const DEFAULT_HOSTLIST: &str = include_str!("../assets/manka-hostlist.txt");

pub fn ensure_exclude_list(paths: &Paths) -> std::io::Result<()> {
    let file = paths.exclude_list();
    if file.exists() {
        return Ok(());
    }
    std::fs::write(file, DEFAULT_EXCLUDE)
}

/// Create `lists/manka-hostlist.txt` once. Contains the resources that are
/// actually blocked and therefore require the DPI bypass.
pub fn ensure_hostlist(paths: &Paths) -> std::io::Result<()> {
    let file = paths.hostlist_file();
    if file.exists() {
        return Ok(());
    }
    std::fs::write(file, DEFAULT_HOSTLIST)
}
