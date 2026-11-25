//! Shared HTTP helpers for component providers and the strategy store.
//!
//! All downloads go over HTTPS to fixed hosts (api.github.com / codeload /
//! release assets). Anything executable is additionally verified against the
//! SHA256 digest published by GitHub before it is unpacked or started.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;

pub const USER_AGENT: &str = concat!("Manka-UI/", env!("CARGO_PKG_VERSION"), " (Windows)");

#[derive(Clone, Debug, Serialize)]
pub struct Progress {
    pub id: String,
    pub stage: String,
    pub downloaded: u64,
    pub total: u64,
    pub percent: u32,
}

pub type ProgressFn<'a> = &'a (dyn Fn(Progress) + Send + Sync);

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("http client")
}

pub async fn get_json(url: &str) -> Result<Value, String> {
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

/// Download a text file (domain lists) with a hard size limit.
pub async fn get_text(url: &str, max_bytes: u64) -> Result<String, String> {
    if !url.starts_with("https://") {
        return Err(format!("Отказ от небезопасного URL: {url}"));
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
    if resp.content_length().unwrap_or(0) > max_bytes {
        return Err(format!("{url}: файл больше {} КБ", max_bytes / 1024));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("{url}: {e}"))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!("{url}: файл больше {} КБ", max_bytes / 1024));
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

pub async fn download(
    url: &str,
    dest: &Path,
    progress: ProgressFn<'_>,
    id: &str,
    stage: &str,
) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err(format!("Отказ от небезопасного URL: {url}"));
    }
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
        downloaded += chunk.len() as u64;
        // zip-bomb / runaway guard: no component archive is anywhere near 1 GiB
        if downloaded > 1024 * 1024 * 1024 {
            drop(file);
            let _ = fs::remove_file(dest);
            return Err(format!("{url}: файл больше 1 ГБ, загрузка прервана"));
        }
        file.write_all(&chunk).map_err(|e| e.to_string())?;
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
