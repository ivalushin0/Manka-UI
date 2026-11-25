//! Component providers: Zapret2 (winws2 + winws bundle) and the headless
//! tg-ws-proxy-rs Telegram proxy.
//!
//! Supply-chain rules:
//! * every URL is HTTPS on a fixed host;
//! * the Zapret bundle is downloaded by full commit SHA (immutable URL), never
//!   by a moving branch ref, so the version label always matches the content;
//! * release assets are verified against the SHA256 digest published by the
//!   GitHub API before anything is unpacked;
//! * the hash of the installed executable is recorded so the Doctor can later
//!   prove the binaries were not tampered with.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::http::{download, get_json};
use crate::model::ComponentStatus;
use crate::paths::{self, Paths};
use crate::util::{sha256_file, verify_sha256};

pub use crate::http::{Progress, ProgressFn};

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

pub(crate) fn extract_zip(
    zip_path: &Path,
    dest: &Path,
    prefix: Option<&str>,
) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    // zip-bomb guard: the biggest legitimate bundle unpacks to ~150 MB
    let mut total_written: u64 = 0;
    const MAX_UNPACKED: u64 = 2 * 1024 * 1024 * 1024;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let raw = entry.name().map_err(|e| e.to_string())?.to_string();
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
            total_written += n as u64;
            if total_written > MAX_UNPACKED {
                return Err("Архив распаковывается в больше 2 ГБ — похоже на zip-бомбу".into());
            }
            std::io::Write::write_all(&mut f, &buf[..n]).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn find_prefix(zip_path: &Path, suffix: &str) -> Result<String, String> {
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().map_err(|e| e.to_string())?.to_string();
        if let Some(idx) = name.find(suffix) {
            return Ok(name[..idx + suffix.len()].to_string());
        }
    }
    Err(format!("в архиве нет папки {suffix}"))
}

// ---------------------------------------------------------------- Zapret bundle

pub const ZAPRET_REPO: &str = "bol-van/zapret-win-bundle";

/// Full SHA of the bundle's `master` head. The installer downloads exactly
/// this commit, so the version label can never drift from the content
/// (no time-of-check/time-of-use window on a moving branch).
pub async fn latest_zapret_commit() -> Result<String, String> {
    let json = get_json(&format!(
        "https://api.github.com/repos/{ZAPRET_REPO}/commits/master"
    ))
    .await?;
    let sha = json["sha"]
        .as_str()
        .ok_or("GitHub: нет sha коммита")?
        .to_string();
    if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("GitHub вернул некорректный sha: {sha}"));
    }
    Ok(sha)
}

/// Short human-facing version label of a bundle commit.
pub fn zapret_version_label(commit: &str) -> String {
    commit.chars().take(12).collect()
}

pub async fn latest_zapret_version() -> Result<String, String> {
    Ok(zapret_version_label(&latest_zapret_commit().await?))
}

pub fn zapret_version(paths: &Paths) -> Option<String> {
    paths::read_current(paths, "zapret").map(|c| c.version)
}

pub fn zapret_dir(paths: &Paths) -> Option<PathBuf> {
    paths.engine_dir()
}

/// Download the `zapret-winws` folder of the upstream bundle at `commit` and
/// switch `current` to it. The previous version stays on disk for rollback.
pub async fn install_zapret(
    paths: &Paths,
    commit: &str,
    progress: ProgressFn<'_>,
    force: bool,
) -> Result<String, String> {
    if commit.len() != 40 || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("Некорректный коммит Zapret: {commit}"));
    }
    let version = zapret_version_label(commit);
    let already = paths::read_current(paths, "zapret").map(|c| c.version);
    if !force && already.as_deref() == Some(version.as_str()) && paths.engine_dir().is_some() {
        return Ok(version);
    }

    let temp = paths.temp();
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let zip_path = temp.join(format!("zapret-bundle-{version}.zip"));
    let need_download = !zip_path.exists()
        || fs::metadata(&zip_path)
            .map(|m| m.len() < 1024)
            .unwrap_or(true);
    if need_download {
        progress(Progress {
            id: "zapret".into(),
            stage: "Скачивание Zapret2".into(),
            downloaded: 0,
            total: 0,
            percent: 0,
        });
        // immutable, content-addressed URL: exactly the commit we resolved
        let url = format!("https://codeload.github.com/{ZAPRET_REPO}/zip/{commit}");
        download(&url, &zip_path, progress, "zapret", "Скачивание Zapret2").await?;
    }
    // GitHub does not publish digests for codeload archives; we record the
    // hash so later reinstalls of the same commit can be compared.
    let archive_sha = sha256_file(&zip_path).map_err(|e| e.to_string())?;

    progress(Progress {
        id: "zapret".into(),
        stage: "Распаковка".into(),
        downloaded: 0,
        total: 0,
        percent: 100,
    });
    let prefix = find_prefix(&zip_path, "zapret-winws/")?;
    let dest = paths.component("zapret").join(&version);
    if dest.exists() {
        let _ = fs::remove_dir_all(&dest);
    }
    extract_zip(&zip_path, &dest, Some(&prefix))?;
    let exe = dest.join("winws2.exe");
    if !exe.exists() {
        return Err("в архиве не найден winws2.exe".into());
    }
    paths::write_current_full(
        paths,
        "zapret",
        &version,
        &version,
        Some(archive_sha),
        Some((&exe, &dest)),
    )
    .map_err(|e| e.to_string())?;

    // keep the current and the previous version only
    cleanup_versions(
        &paths.component("zapret"),
        &version,
        paths::read_current(paths, "zapret").and_then(|c| c.previous),
    );

    let _ = fs::remove_file(&zip_path);
    Ok(version)
}

fn cleanup_versions(dir: &Path, current: &str, previous: Option<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !entry.path().is_dir() || name == current || Some(name) == previous {
            continue;
        }
        let _ = fs::remove_dir_all(entry.path());
    }
}

pub fn rollback_zapret(paths: &Paths) -> Result<String, String> {
    rollback(paths, "zapret", "Zapret")
}

pub fn rollback_tgws(paths: &Paths) -> Result<String, String> {
    rollback(paths, "tgws", "TG WS Proxy")
}

fn rollback(paths: &Paths, id: &str, title: &str) -> Result<String, String> {
    let current = paths::read_current(paths, id).ok_or(format!("{title} не установлен"))?;
    let prev = current.previous.ok_or("Предыдущей версии нет")?;
    let dir = paths.component(id).join(&prev);
    if !dir.exists() {
        return Err("Предыдущая версия удалена".into());
    }
    // recompute the executable hash for the restored version so the Doctor
    // keeps verifying real binaries after a rollback
    let exe = if id == "zapret" {
        let p = dir.join("winws2.exe");
        p.exists().then_some(p)
    } else {
        find_exe_in(&dir)
    };
    paths::write_current_full(
        paths,
        id,
        &prev,
        &prev,
        None,
        exe.as_deref().map(|e| (e, dir.as_path())),
    )
    .map_err(|e| e.to_string())?;
    Ok(prev)
}

// ---------------------------------------------------------------- tg-ws-proxy-rs

pub const TGWS_REPO: &str = "valnesfjord/tg-ws-proxy-rs";

#[derive(serde::Deserialize, Debug, Clone)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    /// GitHub publishes `sha256:<hex>` for every release asset; we require it
    /// and verify the download before unpacking.
    digest: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Release {
    pub tag: String,
    pub asset: String,
    pub url: String,
    pub digest: Option<String>,
}

pub async fn latest_tgws_release() -> Result<Release, String> {
    let json = get_json(&format!(
        "https://api.github.com/repos/{TGWS_REPO}/releases/latest"
    ))
    .await?;
    let tag = json["tag_name"]
        .as_str()
        .ok_or("GitHub: нет tag_name")?
        .to_string();
    let assets: Vec<ReleaseAsset> =
        serde_json::from_value(json["assets"].clone()).unwrap_or_default();
    let pick = assets
        .iter()
        .find(|a| a.name == "tg-ws-proxy-x86_64-pc-windows-gnu.zip")
        .or_else(|| {
            assets
                .iter()
                .find(|a| a.name == "tg-ws-proxy-x86_64-win7-windows-msvc.zip")
        })
        .or_else(|| {
            assets
                .iter()
                .find(|a| a.name.contains("x86_64") && a.name.contains("windows"))
        })
        .ok_or("В релизе нет сборки для Windows x64")?;
    let digest = pick.digest.clone().filter(|d| d.len() > "sha256:".len());
    Ok(Release {
        tag,
        asset: pick.name.clone(),
        url: pick.browser_download_url.clone(),
        digest,
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
    download(
        &release.url,
        &zip_path,
        progress,
        "tgws",
        "Скачивание TG WS Proxy",
    )
    .await?;
    // Refuse to install anything that does not match the digest GitHub
    // published for this exact asset: a swapped binary would otherwise run
    // with administrator rights.
    let digest = release
        .digest
        .as_deref()
        .ok_or("GitHub не вернул SHA256 для ассета — установка отменена")?;
    verify_sha256(&zip_path, digest)?;
    let archive_sha = sha256_file(&zip_path).map_err(|e| e.to_string())?;

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
    let exe = find_exe_in(&dest).ok_or("В архиве не найден tg-ws-proxy.exe")?;
    paths::write_current_full(
        paths,
        "tgws",
        &release.tag,
        &release.tag,
        Some(archive_sha),
        Some((&exe, &dest)),
    )
    .map_err(|e| e.to_string())?;
    cleanup_versions(
        &paths.component("tgws"),
        &release.tag,
        paths::read_current(paths, "tgws").and_then(|c| c.previous),
    );
    let _ = fs::remove_file(&zip_path);
    Ok(release.tag.clone())
}

fn find_exe_in(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Some(found) = find_exe_in(&p) {
                return Some(found);
            }
        } else if p
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_lowercase().ends_with(".exe"))
            .unwrap_or(false)
        {
            return Some(p);
        }
    }
    None
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
            path: paths.tgws_exe().map(|p| p.to_string_lossy().to_string()),
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

// ---------------------------------------------------------------- integrity

#[derive(Debug, Clone, serde::Serialize)]
pub struct IntegrityCheck {
    pub id: String,
    /// true — hash matches; false — mismatch; None — no baseline recorded yet
    pub ok: Option<bool>,
    pub detail: String,
}

/// Re-hash the installed executables and compare them with the values recorded
/// at install time. Detects binaries replaced after installation (the classic
/// "антивирус ругается — это точно наша сборка?" question).
pub fn verify_installed(paths: &Paths) -> Vec<IntegrityCheck> {
    let mut out = Vec::new();
    for (id, title) in [("zapret", "Zapret (winws2.exe)"), ("tgws", "TG WS Proxy")] {
        let Some(cur) = paths::read_current(paths, id) else {
            out.push(IntegrityCheck {
                id: id.into(),
                ok: None,
                detail: format!("{title}: не установлен"),
            });
            continue;
        };
        let (Some(expected), Some(rel)) = (cur.exe_sha256.clone(), cur.exe_rel.clone()) else {
            out.push(IntegrityCheck {
                id: id.into(),
                ok: None,
                detail: format!("{title}: контрольная сумма не записана (установлено старой версией) — переустановите компонент"),
            });
            continue;
        };
        let exe = paths.component(id).join(&cur.dir).join(&rel);
        match sha256_file(&exe) {
            Ok(actual) if actual.eq_ignore_ascii_case(&expected) => out.push(IntegrityCheck {
                id: id.into(),
                ok: Some(true),
                detail: format!("{title}: целостность подтверждена ({}…)", &actual[..12]),
            }),
            Ok(actual) => out.push(IntegrityCheck {
                id: id.into(),
                ok: Some(false),
                detail: format!(
                    "{title}: ХЭШ НЕ СОВПАДАЕТ (был {expected}, стал {actual}). Переустановите компонент!"
                ),
            }),
            Err(e) => out.push(IntegrityCheck {
                id: id.into(),
                ok: Some(false),
                detail: format!("{title}: не удалось прочитать {rel}: {e}"),
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn zip_entry_name_blocks_traversal() {
        assert!(zip_entry_name("dir/file.txt").is_some());
        assert!(zip_entry_name("file.txt").is_some());
        assert!(zip_entry_name("../evil.exe").is_none());
        assert!(zip_entry_name("a/../../evil.exe").is_none());
        assert!(zip_entry_name("a/./b.txt").is_some());
        // absolute paths are rejected
        assert!(zip_entry_name("C:/Windows/evil.exe").is_none());
        assert!(zip_entry_name("/etc/passwd").is_none());
    }

    #[test]
    fn extract_zip_writes_only_inside_dest() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("test.zip");
        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("ok.txt", opts).unwrap();
            zip.write_all(b"fine").unwrap();
            zip.start_file("../evil.txt", opts).unwrap();
            zip.write_all(b"bad").unwrap();
            zip.finish().unwrap();
        }
        let dest = dir.path().join("out");
        extract_zip(&zip_path, &dest, None).unwrap();
        assert!(dest.join("ok.txt").exists());
        assert!(!dir.path().join("evil.txt").exists());
    }
}
