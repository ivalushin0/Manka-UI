//! HTTPS probes: the same idea as the mobile app — a full TLS handshake and
//! the first 64 KiB of the body, which catches the "freeze after 16–20 KB"
//! kind of blocking.

use std::time::{Duration, Instant};

use futures_util::{stream, StreamExt};
use reqwest::redirect::Policy;
use reqwest::Client;

use crate::model::SiteResult;

const LIMIT: u64 = 64 * 1024;
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

pub fn client(timeout_sec: u32) -> Client {
    let timeout = Duration::from_secs(timeout_sec.max(1) as u64);
    Client::builder()
        .user_agent(UA)
        .connect_timeout(timeout)
        .timeout(timeout)
        .redirect(Policy::none())
        .pool_max_idle_per_host(0)
        .danger_accept_invalid_certs(false)
        .build()
        .expect("probe client")
}

/// Probe every URL `requests` times with at most `concurrency` URLs at once.
pub async fn check_sites(
    urls: &[String],
    requests: u32,
    timeout_sec: u32,
    concurrency: usize,
) -> Vec<SiteResult> {
    let requests = requests.max(1);
    let client = client(timeout_sec);
    let mut indexed = stream::iter(urls.iter().cloned().enumerate())
        .map(|(idx, url)| {
            let client = client.clone();
            async move {
                let url = if url.starts_with("http://") || url.starts_with("https://") {
                    url.clone()
                } else {
                    format!("https://{url}")
                };
                let mut ok = 0u32;
                let mut total_ms = 0u64;
                let mut last_error = None;
                let mut last_status = None;
                let mut bytes = 0u64;
                for _ in 0..requests {
                    let started = Instant::now();
                    match once(&client, &url).await {
                        Ok((status, read)) => {
                            ok += 1;
                            total_ms += started.elapsed().as_millis() as u64;
                            last_status = Some(status);
                            bytes = read;
                        }
                        Err(e) => last_error = Some(e),
                    }
                }
                (
                    idx,
                    SiteResult {
                        url,
                        ok,
                        total: requests,
                        avg_ms: if ok > 0 { total_ms / ok as u64 } else { 0 },
                        status: last_status,
                        bytes,
                        error: if ok < requests { last_error } else { None },
                    },
                )
            }
        })
        .buffer_unordered(concurrency.max(1))
        .collect::<Vec<_>>()
        .await;
    indexed.sort_by_key(|(idx, _)| *idx);
    indexed.into_iter().map(|(_, r)| r).collect()
}

async fn once(client: &Client, url: &str) -> Result<(u16, u64), String> {
    let mut resp = client
        .get(url)
        .header("Connection", "close")
        .header("Accept-Encoding", "identity")
        .send()
        .await
        .map_err(short_err)?;
    let status = resp.status().as_u16();
    let declared = resp.content_length();
    let mut read = 0u64;
    while read < LIMIT {
        match resp.chunk().await {
            Ok(Some(chunk)) => read += chunk.len() as u64,
            Ok(None) => break,
            Err(e) => return Err(short_err(e)),
        }
    }
    if let Some(len) = declared {
        if len > 0 && read < len.min(LIMIT) {
            return Err(format!("обрезано ({read}/{len})"));
        }
    }
    Ok((status, read))
}

fn short_err(e: reqwest::Error) -> String {
    let mut text = e.to_string();
    if text.len() > 160 {
        text.truncate(160);
    }
    text
}

/// Aggregated score of a probe set: success rate first, then average latency.
pub fn score(results: &[SiteResult]) -> (u32, u64) {
    let ok: u32 = results.iter().map(|r| r.ok).sum();
    let total: u32 = results.iter().map(|r| r.total).sum();
    let percent = ok.saturating_mul(100).checked_div(total).unwrap_or(0);
    let ms: Vec<u64> = results
        .iter()
        .filter(|r| r.ok > 0)
        .map(|r| r.avg_ms)
        .collect();
    let avg = if ms.is_empty() {
        0
    } else {
        ms.iter().sum::<u64>() / ms.len() as u64
    };
    (percent, avg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_aggregates() {
        let r = vec![
            SiteResult {
                url: "a".into(),
                ok: 1,
                total: 2,
                avg_ms: 100,
                status: Some(200),
                bytes: 10,
                error: None,
            },
            SiteResult {
                url: "b".into(),
                ok: 2,
                total: 2,
                avg_ms: 300,
                status: Some(200),
                bytes: 10,
                error: None,
            },
        ];
        assert_eq!(score(&r), (75, 200));
    }
}
