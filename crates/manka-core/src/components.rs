//! Component providers: Zapret2 (winws2 + winws bundle) and the headless
//! tg-ws-proxy-rs Telegram proxy.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zip::ZipArchive;

use crate::model::ComponentStatus;
use crate::paths::{self, Paths};

const USER_AGENT: &str = "Manka-UI/0.1 (Windows)";

#[derive(Clone, Debug, Serialize)]
pub struct Progress {
    pub id: String,
    pub stage: String,
    pub downloaded: u64,
    pub total: u64,
    pub percent: u32,
}

pub type ProgressFn<'a> = &'a (dyn Fn(Progress) + Send + Sync);

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("http client")
}

async fn get_json(url: &str) -> Result<Value, String> {
    let resp = client()
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("{url}: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("{url}: HTTP {status}"));
    }
    resp.json::<Value>()
        .await
        .map_err(|e| format!("{url}: {e}"))
}

pub(crate) async fn download(url: &str, dest: &Path, progress: ProgressFn<'_>, id: &str, stage: &str) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let resp = client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("{url}: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("{url}: HTTP {status}"));
    }
    let total = resp.content_length().unwrap_or(0);
    let mut stream = resp.bytes_stream();
    let mut file = File::create(dest).map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut last_pct: u32 = 101;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("{url}: {e}"))?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        let pct = if total > 0 {
            ((downloaded as f64 / total as f64) * 100.0) as u32
        } else {
            0
        };
        if pct != last_pct {
            last_pct = pct;
            progress(Progress {
                id: id.to_string(),
                stage: stage.to_string(),
                downloaded,
                total,
                percent: pct,
            });
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    Ok(())
}

fn zip_entry_name(name: &str) -> Option<PathBuf> {
    // zip-slip guard: only relative normal components
    let path = Path::new(name);
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Normal(c) => out.push(c),
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    Some(out)
}

pub(crate) fn extract_zip(zip_path: &Path, dest: &Path, prefix: Option<&str>) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let raw = entry.name().to_string();
        let rel = match prefix {
            Some(p) => match raw.strip_prefix(p) {
                Some(rest) => rest,
                None => continue,
            },
            None => raw.as_str(),
        };
        if rel.is_empty() {
            continue;
        }
        let Some(mut out) = zip_entry_name(rel) else {
            continue;
        };
        out = dest.join(out);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut f = File::create(&out).map_err(|e| e.to_string())?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = entry.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn find_prefix(zip_path: &Path, suffix: &str) -> Result<String, String> {
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if let Some(idx) = name.find(suffix) {
            return Ok(name[..idx + suffix.len()].to_string());
        }
    }
    Err(format!("в архиве нет папки {suffix}"))
}

// ---------------------------------------------------------------- Zapret bundle

pub const ZAPRET_REPO: &str = "bol-van/zapret-win-bundle";

pub async fn latest_zapret_version() -> Result<String, String> {
    let json = get_json(&format!(
        "https://api.github.com/repos/{ZAPRET_REPO}/commits/master"
    ))
    .await?;
    let sha = json["sha"].as_str().ok_or("GitHub: нет sha коммита")?;
    Ok(sha.chars().take(12).collect())
}

pub fn zapret_version(paths: &Paths) -> Option<String> {
    paths::read_current(paths, "zapret").map(|c| c.version)
}

pub fn zapret_dir(paths: &Paths) -> Option<PathBuf> {
    paths.engine_dir()
}

/// Download the `zapret-winws` folder of the upstream bundle and switch
/// `current` to it. The previous version stays on disk for rollback.
pub async fn install_zapret(
    paths: &Paths,
    version: &str,
    progress: ProgressFn<'_>,
    force: bool,
) -> Result<String, String> {
    let already = paths::read_current(paths, "zapret").map(|c| c.version);
    if !force && already.as_deref() == Some(version)
        && paths.engine_dir().is_some() {
            return Ok(version.to_string());
        }

    let temp = paths.temp();
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let zip_path = temp.join(format!("zapret-bundle-{version}.zip"));
    let need_download = !zip_path.exists() || fs::metadata(&zip_path).map(|m| m.len() < 1024).unwrap_or(true);
    if need_download {
        progress(Progress {
            id: "zapret".into(),
            stage: "Скачивание Zapret2".into(),
            downloaded: 0,
            total: 0,
            percent: 0,
        });
        let url = format!(
            "https://codeload.github.com/{ZAPRET_REPO}/zip/refs/heads/master"
        );
        download(&url, &zip_path, progress, "zapret", "Скачивание Zapret2").await?;
    }

    progress(Progress {
        id: "zapret".into(),
        stage: "Распаковка".into(),
        downloaded: 0,
        total: 0,
        percent: 100,
    });
    let prefix = find_prefix(&zip_path, "zapret-winws/")?;
    let dest = paths.component("zapret").join(version);
    if dest.exists() {
        let _ = fs::remove_dir_all(&dest);
    }
    extract_zip(&zip_path, &dest, Some(&prefix))?;
    if !dest.join("winws2.exe").exists() {
        return Err("в архиве не найден winws2.exe".into());
    }
    paths::write_current(paths, "zapret", version, version).map_err(|e| e.to_string())?;

    // keep the current and the previous version only
    cleanup_versions(&paths.component("zapret"), version, paths::read_current(paths, "zapret").and_then(|c| c.previous));

    let _ = fs::remove_file(&zip_path);
    Ok(version.to_string())
}

fn cleanup_versions(dir: &Path, current: &str, previous: Option<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !entry.path().is_dir() || name == current || Some(name) == previous {
            continue;
        }
        let _ = fs::remove_dir_all(entry.path());
    }
}

pub fn rollback_zapret(paths: &Paths) -> Result<String, String> {
    let current = paths::read_current(paths, "zapret").ok_or("Zapret не установлен")?;
    let prev = current.previous.ok_or("Предыдущей версии нет")?;
    let dir = paths.component("zapret").join(&prev);
    if !dir.exists() {
        return Err("Предыдущая версия удалена".into());
    }
    paths::write_current(paths, "zapret", &prev, &prev).map_err(|e| e.to_string())?;
    Ok(prev)
}

// ---------------------------------------------------------------- tg-ws-proxy-rs

pub const TGWS_REPO: &str = "valnesfjord/tg-ws-proxy-rs";

#[derive(Deserialize, Debug, Clone)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone)]
pub struct Release {
    pub tag: String,
    pub asset: String,
    pub url: String,
}

pub async fn latest_tgws_release() -> Result<Release, String> {
    let json = get_json(&format!(
        "https://api.github.com/repos/{TGWS_REPO}/releases/latest"
    ))
    .await?;
    let tag = json["tag_name"].as_str().ok_or("GitHub: нет tag_name")?.to_string();
    let assets: Vec<ReleaseAsset> = serde_json::from_value(json["assets"].clone()).unwrap_or_default();
    let pick = assets
        .iter()
        .find(|a| a.name == "tg-ws-proxy-x86_64-pc-windows-gnu.zip")
        .or_else(|| assets.iter().find(|a| a.name == "tg-ws-proxy-x86_64-win7-windows-msvc.zip"))
        .or_else(|| assets.iter().find(|a| a.name.contains("x86_64") && a.name.contains("windows")))
        .ok_or("В релизе нет сборки для Windows x64")?;
    Ok(Release {
        tag,
        asset: pick.name.clone(),
        url: pick.browser_download_url.clone(),
    })
}

pub fn tgws_version(paths: &Paths) -> Option<String> {
    paths::read_current(paths, "tgws").map(|c| c.version)
}

pub async fn install_tgws(
    paths: &Paths,
    release: &Release,
    progress: ProgressFn<'_>,
    force: bool,
) -> Result<String, String> {
    let already = tgws_version(paths);
    if !force && already.as_deref() == Some(release.tag.as_str()) && paths.tgws_exe().is_some() {
        return Ok(release.tag.clone());
    }

    let temp = paths.temp();
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let zip_path = temp.join(format!("tgws-{}.zip", release.tag));
    progress(Progress {
        id: "tgws".into(),
        stage: "Скачивание TG WS Proxy".into(),
        downloaded: 0,
        total: 0,
        percent: 0,
    });
    download(&release.url, &zip_path, progress, "tgws", "Скачивание TG WS Proxy").await?;

    progress(Progress {
        id: "tgws".into(),
        stage: "Распаковка".into(),
        downloaded: 0,
        total: 0,
        percent: 100,
    });
    let dest = paths.component("tgws").join(&release.tag);
    if dest.exists() {
        let _ = fs::remove_dir_all(&dest);
    }
    extract_zip(&zip_path, &dest, None)?;
    if paths
        .tgws_exe()
        .is_none()
    {
        // find before switching, using the new dir directly
        if !contains_exe(&dest) {
            return Err("В архиве не найден tg-ws-proxy.exe".into());
        }
    }
    paths::write_current(paths, "tgws", &release.tag, &release.tag).map_err(|e| e.to_string())?;
    cleanup_versions(
        &paths.component("tgws"),
        &release.tag,
        paths::read_current(paths, "tgws").and_then(|c| c.previous),
    );
    let _ = fs::remove_file(&zip_path);
    Ok(release.tag.clone())
}

fn contains_exe(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else { return false };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if contains_exe(&p) {
                return true;
            }
        } else if p
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_lowercase().ends_with(".exe"))
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------- status

pub fn status(paths: &Paths) -> Vec<ComponentStatus> {
    let z_cur = paths::read_current(paths, "zapret");
    let t_cur = paths::read_current(paths, "tgws");
    let engine_dir = paths.engine_dir();
    vec![
        ComponentStatus {
            id: "zapret".into(),
            name: "Zapret 2 (winws2 + winws)".into(),
            kind: "engine".into(),
            installed: engine_dir.is_some(),
            version: z_cur.as_ref().map(|c| c.version.clone()),
            latest: None,
            update_available: false,
            path: engine_dir.map(|p| p.to_string_lossy().to_string()),
            note: Some("Официальный бандл bol-van/zapret-win-bundle".into()),
        },
        ComponentStatus {
            id: "tgws".into(),
            name: "TG WS Proxy (Rust, headless)".into(),
            kind: "proxy".into(),
            installed: paths.tgws_exe().is_some(),
            version: t_cur.as_ref().map(|c| c.version.clone()),
            latest: None,
            update_available: false,
            path: paths
                .tgws_exe()
                .map(|p| p.to_string_lossy().to_string()),
            note: Some("MTProto-прокси без трея, 127.0.0.1:1443".into()),
        },
    ]
}

pub async fn check_updates(paths: &Paths) -> Vec<ComponentStatus> {
    let mut list = status(paths);
    let latest_z = latest_zapret_version().await.ok();
    let latest_t = latest_tgws_release().await.ok().map(|r| r.tag);
    for item in &mut list {
        let (latest, current) = match item.id.as_str() {
            "zapret" => (latest_z.clone(), item.version.clone()),
            "tgws" => (latest_t.clone(), item.version.clone()),
            _ => (None, None),
        };
        item.latest = latest.clone();
        item.update_available = match (&latest, &current) {
            (Some(l), Some(c)) => l != c,
            (Some(_), None) => true,
            _ => false,
        };
    }
    list
}
