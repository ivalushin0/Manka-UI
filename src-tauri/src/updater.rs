use std::time::Duration;

use serde::Serialize;
use tauri_plugin_updater::UpdaterExt;

pub(crate) fn github_repo(repo: &str) -> Option<String> {
    let mut repo = repo.trim();
    for prefix in ["https://github.com/", "http://github.com/", "github.com/"] {
        if let Some(rest) = repo.strip_prefix(prefix) {
            repo = rest;
            break;
        }
    }
    let repo = repo.trim_end_matches('/');
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    let mut parts = repo.split('/');
    let owner = parts.next()?;
    let name = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let valid = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    if valid(owner) && valid(name) {
        Some(format!("{owner}/{name}"))
    } else {
        None
    }
}

pub(crate) fn build_updater(
    app: &tauri::AppHandle,
    repo: &str,
) -> Result<tauri_plugin_updater::Updater, String> {
    let repo = github_repo(repo)
        .ok_or("Укажите GitHub-репозиторий в формате owner/repo (например, ivalu/manka)")?;
    let endpoint = format!("https://github.com/{repo}/releases/latest/download/latest.json");
    let url = url::Url::parse(&endpoint).map_err(|e| e.to_string())?;
    app.updater_builder()
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct AppUpdateInfo {
    pub current_version: String,
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
    pub download_url: String,
    pub releases_url: String,
}
