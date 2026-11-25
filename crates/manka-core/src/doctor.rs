//! «Доктор» — диагностика в один клик.
//!
//! Собирает состояние системы, которое в аналогичных проектах составляет 90%
//! issues («не работает», «антивирус ругается», «после обновления сломалось»):
//! права, конфликтующие обходы, драйвер WinDivert, целостность бинарников,
//! занятые порты, автозапуск, облачные папки. Умеет экспортировать всё в zip
//! для прикрепления к issue.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;

use serde::Serialize;

use crate::paths::Paths;
use crate::settings::Settings;
use crate::util;

#[derive(Serialize, Clone, Debug)]
pub struct DoctorCheck {
    pub id: String,
    pub title: String,
    /// "ok" | "warn" | "fail"
    pub status: String,
    pub detail: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct DoctorReport {
    pub checks: Vec<DoctorCheck>,
    pub generated_at: i64,
    pub ok: u32,
    pub warn: u32,
    pub fail: u32,
}

impl DoctorReport {
    fn push(&mut self, id: &str, title: &str, status: &str, detail: impl Into<String>) {
        match status {
            "ok" => self.ok += 1,
            "warn" => self.warn += 1,
            _ => self.fail += 1,
        }
        self.checks.push(DoctorCheck {
            id: id.into(),
            title: title.into(),
            status: status.into(),
            detail: detail.into(),
        });
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "Manka Doctor — {} (ok: {}, warn: {}, fail: {})",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            self.ok,
            self.warn,
            self.fail
        );
        for c in &self.checks {
            let _ = writeln!(
                out,
                "[{:<4}] {}: {}",
                c.status.to_uppercase(),
                c.title,
                c.detail
            );
        }
        out
    }
}

/// DPI-инструменты, которые перехватывают тот же трафик и конфликтуют с Manka.
const CONFLICTING: &[&str] = &[
    "winws.exe",
    "winws2.exe",
    "ciadpi.exe",
    "byedpi.exe",
    "goodbyedpi.exe",
    "goodbyedpi-0.2.3rc1.exe",
    "spoofdpi.exe",
    "tpws.exe",
    "nfqws.exe",
    "nfqws2.exe",
    "TgWsProxy.exe",
    "tg-ws-proxy.exe",
];

fn cloud_sync_path(path: &str) -> Option<&'static str> {
    let p = path.to_lowercase();
    for (needle, name) in [
        ("onedrive", "OneDrive"),
        ("dropbox", "Dropbox"),
        ("google drive", "Google Drive"),
        ("googledrive", "Google Drive"),
        ("yandexdisk", "Яндекс.Диск"),
        ("yandex disk", "Яндекс.Диск"),
        ("pcloud", "pCloud"),
        ("nextcloud", "Nextcloud"),
    ] {
        if p.contains(needle) {
            return Some(name);
        }
    }
    None
}

pub fn run(paths: &Paths, settings: &Settings) -> DoctorReport {
    let mut r = DoctorReport {
        generated_at: util::now_ms(),
        ..Default::default()
    };

    // 1. права администратора
    if util::is_elevated() {
        r.push(
            "elevated",
            "Права администратора",
            "ok",
            "Manka запущена от имени администратора",
        );
    } else {
        r.push(
            "elevated",
            "Права администратора",
            "fail",
            "Manka запущена без прав администратора — WinDivert не сможет перехватывать трафик. Перезапустите от имени администратора.",
        );
    }

    // 2. компоненты установлены
    match paths.engine_dir() {
        Some(dir) => {
            let version = crate::paths::read_current(paths, "zapret")
                .map(|c| c.version)
                .unwrap_or_else(|| "?".into());
            r.push(
                "engine",
                "Движок Zapret",
                "ok",
                format!("установлен ({version}), {}", dir.display()),
            );
        }
        None => r.push(
            "engine",
            "Движок Zapret",
            "fail",
            "не установлен — откройте «Компоненты» и установите Zapret 2",
        ),
    }

    // 3. целостность бинарников (не подменены после установки?)
    for check in crate::components::verify_installed(paths) {
        let status = match check.ok {
            Some(true) => "ok",
            Some(false) => "fail",
            None => "warn",
        };
        r.push(
            &format!("integrity-{}", check.id),
            "Целостность файлов",
            status,
            check.detail,
        );
    }

    // 4. конфликтующие процессы (чужие обходы и дубликаты)
    let components_root = paths.components().to_string_lossy().to_lowercase();
    let mut foreign: Vec<String> = Vec::new();
    let mut ours = 0u32;
    for (pid, name, path) in util::find_processes(CONFLICTING) {
        let ours_proc = !path.is_empty() && path.to_lowercase().starts_with(&components_root);
        if ours_proc {
            ours += 1;
        } else {
            foreign.push(format!("{name} (PID {pid})"));
        }
    }
    if foreign.is_empty() {
        r.push(
            "conflicts",
            "Конфликтующие программы",
            "ok",
            format!("чужих обходов не найдено; процессов Manka: {ours}"),
        );
    } else {
        r.push(
            "conflicts",
            "Конфликтующие программы",
            "warn",
            format!(
                "найдены другие DPI-инструменты: {}. Два перехватчика одновременно ломают друг друга — закройте лишние.",
                foreign.join(", ")
            ),
        );
    }

    // 5. драйвер WinDivert
    match util::service_state("windivert").or_else(|| util::service_state("windivert2")) {
        Some(state) => {
            let status = if state.eq_ignore_ascii_case("RUNNING") {
                "ok"
            } else {
                "warn"
            };
            r.push(
                "windivert",
                "Драйвер WinDivert",
                status,
                format!("состояние: {state}"),
            );
        }
        None => r.push(
            "windivert",
            "Драйвер WinDivert",
            "ok",
            "драйвер не загружен (появится при запуске обхода)",
        ),
    }

    // 6. порт Telegram-прокси
    if settings.tgws_enabled {
        match util::port_owner(settings.tgws_port) {
            Some((pid, name)) => {
                let lower = name.to_lowercase();
                if lower.contains("tg-ws-proxy") || lower.contains("tgws") {
                    r.push(
                        "tgport",
                        "Порт Telegram-прокси",
                        "ok",
                        format!("{} слушает {name} (PID {pid})", settings.tgws_port),
                    );
                } else {
                    r.push(
                        "tgport",
                        "Порт Telegram-прокси",
                        "fail",
                        format!(
                            "порт {} занят программой {name} (PID {pid}) — прокси не сможет запуститься. Смените порт в настройках Telegram.",
                            settings.tgws_port
                        ),
                    );
                }
            }
            None => r.push(
                "tgport",
                "Порт Telegram-прокси",
                "warn",
                format!(
                    "прокси включён, но порт {} никем не слушается — похоже, прокси упал",
                    settings.tgws_port
                ),
            ),
        }
    }

    // 7. данные в облачной синхронизации
    let data_dir = paths.root().to_string_lossy().to_string();
    if let Some(cloud) = cloud_sync_path(&data_dir) {
        r.push(
            "clouddir",
            "Папка данных",
            "warn",
            format!(
                "данные лежат в синхронизируемой папке {cloud}: {data_dir}. Облако может блокировать файлы при обновлении. Перенесите данные (MANKA_DATA_DIR) в локальную папку."
            ),
        );
    } else {
        r.push("clouddir", "Папка данных", "ok", data_dir);
    }

    // 8. автозапуск
    if settings.autostart {
        let task = std::process::Command::new("schtasks")
            .args(["/Query", "/TN", "Manka"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        let reg = std::process::Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                "Manka",
            ])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if task || reg {
            r.push(
                "autostart",
                "Автозапуск",
                "ok",
                if task {
                    "задача планировщика «Manka» создана"
                } else {
                    "запись в реестре Run создана"
                },
            );
        } else {
            r.push(
                "autostart",
                "Автозапуск",
                "warn",
                "автозапуск включён в настройках, но ни задачи планировщика, ни записи в реестре не найдено — выключите и включите его заново",
            );
        }
    }

    // 9. списки
    let hostlist = crate::lists::effective_hostlist(paths);
    let domains = std::fs::read_to_string(&hostlist)
        .map(|t| t.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);
    if domains > 0 {
        r.push(
            "hostlist",
            "Списки доменов",
            "ok",
            format!("{domains} доменов в {hostlist:?}"),
        );
    } else {
        r.push(
            "hostlist",
            "Списки доменов",
            "warn",
            "hostlist пуст — в режиме «по списку» обход не будет применяться ни к чему",
        );
    }

    r
}

/// Zip everything needed for a bug report: doctor text, logs, versions and a
/// redacted copy of the settings. Returns the archive path.
pub fn export(paths: &Paths, settings: &Settings) -> Result<PathBuf, String> {
    let report = run(paths, settings);
    let dir = paths.results();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let zip_path = dir.join(format!("manka-diagnostics-{stamp}.zip"));
    let file = std::fs::File::create(&zip_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();

    let add =
        |zip: &mut zip::ZipWriter<std::fs::File>, name: &str, data: &[u8]| -> Result<(), String> {
            zip.start_file(name, opts).map_err(|e| e.to_string())?;
            zip.write_all(data).map_err(|e| e.to_string())
        };

    let mut header = String::new();
    let _ = writeln!(header, "Manka {}", crate::VERSION);
    let _ = writeln!(header, "OS: {}", std::env::consts::OS);
    let _ = writeln!(header, "data dir: {}", paths.root().display());
    add(&mut zip, "about.txt", header.as_bytes())?;
    add(&mut zip, "doctor.txt", report.to_text().as_bytes())?;

    // settings with the proxy secret redacted
    let mut redacted = settings.clone();
    redacted.tgws_secret = "***".into();
    let settings_json = serde_json::to_vec_pretty(&redacted).map_err(|e| e.to_string())?;
    add(&mut zip, "settings.json", &settings_json)?;

    for (name, file) in [
        ("logs/app.log", paths.logs().join("app.log")),
        ("logs/engine.log", paths.logs().join("engine.log")),
        ("logs/tgws.log", paths.logs().join("tgws.log")),
    ] {
        if let Ok(data) = std::fs::read(&file) {
            // logs are capped so the archive stays small
            let tail = if data.len() > 512 * 1024 {
                &data[data.len() - 512 * 1024..]
            } else {
                &data[..]
            };
            add(&mut zip, name, tail)?;
        }
    }
    for id in ["zapret", "tgws"] {
        let current = paths.current_file(id);
        if let Ok(data) = std::fs::read(&current) {
            add(&mut zip, &format!("components/{id}-current.json"), &data)?;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(zip_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_paths_are_detected() {
        assert_eq!(
            cloud_sync_path(r"C:\Users\x\OneDrive\Manka"),
            Some("OneDrive")
        );
        assert_eq!(cloud_sync_path(r"D:\Dropbox\apps"), Some("Dropbox"));
        assert_eq!(cloud_sync_path(r"C:\Users\x\AppData\Local\MankaData"), None);
    }

    #[test]
    fn report_text_lists_all_checks() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let settings = Settings::default();
        let report = run(&paths, &settings);
        assert!(!report.checks.is_empty());
        let text = report.to_text();
        assert!(text.contains("Manka Doctor"));
        // engine is not installed in the temp dir → at least one fail
        assert!(report.fail >= 1);
    }

    #[test]
    fn export_redacts_secret() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        paths.ensure().unwrap();
        let settings = Settings::default();
        let secret = settings.tgws_secret.clone();
        let zip_path = export(&paths, &settings).unwrap();
        assert!(zip_path.exists());
        let data = std::fs::read(&zip_path).unwrap();
        // the secret must not appear anywhere in the archive
        assert!(!String::from_utf8_lossy(&data).contains(&secret));
    }
}
