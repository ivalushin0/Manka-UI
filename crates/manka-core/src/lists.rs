//! External domain lists with mirrors and scheduled refresh.
//!
//! The effective hostlist the engine uses is the union of the built-in
//! `manka-hostlist.txt` and downloaded remote lists, de-duplicated into
//! `effective-hostlist.txt`. Banks/gov services stay protected by
//! `manka-exclude.txt`, which is applied separately.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde::Serialize;

use crate::http;
use crate::paths::Paths;
use crate::settings::Settings;

/// A list source: display name plus URLs tried in order (primary, mirrors).
pub struct ListSource {
    pub name: &'static str,
    pub urls: &'static [&'static str],
}

/// Built-in sources. Re-filter is the de-facto community list; jsDelivr is a
/// mirror for networks where raw.githubusercontent.com is blocked.
pub const DEFAULT_SOURCES: &[ListSource] = &[ListSource {
    name: "Re-filter (список доменов)",
    urls: &[
        "https://raw.githubusercontent.com/1andrevich/Re-filter-lists/main/domains_all.lst",
        "https://cdn.jsdelivr.net/gh/1andrevich/Re-filter-lists@main/domains_all.lst",
    ],
}];

const MAX_LIST_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, Default)]
pub struct ListsReport {
    pub sources_ok: u32,
    pub sources_failed: u32,
    pub domains: u32,
    pub errors: Vec<String>,
    pub updated_at: i64,
}

pub fn remote_list_file(paths: &Paths) -> PathBuf {
    paths.lists().join("remote-hostlist.txt")
}

pub fn effective_hostlist_file(paths: &Paths) -> PathBuf {
    paths.lists().join("effective-hostlist.txt")
}

/// The hostlist the engine should actually use: the merged file when it
/// exists, otherwise the built-in one.
pub fn effective_hostlist(paths: &Paths) -> PathBuf {
    let merged = effective_hostlist_file(paths);
    if merged.exists() {
        merged
    } else {
        paths.hostlist_file()
    }
}

/// Keep lines that look like hostnames: no comments, no blanks, no spaces.
fn normalize(text: &str, into: &mut BTreeSet<String>) {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with('!')
            || line.starts_with("//")
        {
            continue;
        }
        let line = line
            .trim_start_matches("+.")
            .trim_start_matches("*.")
            .to_lowercase();
        if line.len() > 253 || line.contains(char::is_whitespace) {
            continue;
        }
        if line
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            && line.contains('.')
        {
            into.insert(line);
        }
    }
}

/// Rebuild `effective-hostlist.txt` from the built-in list plus remote lists.
pub fn rebuild_effective(paths: &Paths) -> std::io::Result<u32> {
    let mut domains = BTreeSet::new();
    if let Ok(text) = fs::read_to_string(paths.hostlist_file()) {
        normalize(&text, &mut domains);
    }
    if let Ok(text) = fs::read_to_string(remote_list_file(paths)) {
        normalize(&text, &mut domains);
    }
    let body: String = domains.iter().map(|d| format!("{d}\n")).collect();
    crate::util::atomic_write(&effective_hostlist_file(paths), body.as_bytes())?;
    Ok(domains.len() as u32)
}

fn user_sources(settings: &Settings) -> Vec<String> {
    settings
        .lists_sources
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter(|l| l.starts_with("https://"))
        .map(str::to_string)
        .collect()
}

/// Download all sources (built-in with mirrors + user URLs), merge and
/// rebuild the effective hostlist.
pub async fn update(paths: &Paths, settings: &Settings) -> ListsReport {
    let mut report = ListsReport::default();
    let mut domains = BTreeSet::new();

    for source in DEFAULT_SOURCES {
        let mut last_err = String::new();
        let mut ok = false;
        for url in source.urls {
            match http::get_text(url, MAX_LIST_BYTES).await {
                Ok(text) => {
                    normalize(&text, &mut domains);
                    ok = true;
                    break;
                }
                Err(e) => last_err = e,
            }
        }
        if ok {
            report.sources_ok += 1;
        } else {
            report.sources_failed += 1;
            report.errors.push(format!("{}: {last_err}", source.name));
        }
    }

    for url in user_sources(settings) {
        match http::get_text(&url, MAX_LIST_BYTES).await {
            Ok(text) => {
                normalize(&text, &mut domains);
                report.sources_ok += 1;
            }
            Err(e) => {
                report.sources_failed += 1;
                report.errors.push(e);
            }
        }
    }

    if report.sources_ok > 0 {
        let body: String = domains.iter().map(|d| format!("{d}\n")).collect();
        let _ = crate::util::atomic_write(&remote_list_file(paths), body.as_bytes());
        match rebuild_effective(paths) {
            Ok(total) => report.domains = total,
            Err(e) => report.errors.push(format!("effective-hostlist: {e}")),
        }
        report.updated_at = crate::util::now_ms();
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_filters_junk() {
        let mut set = BTreeSet::new();
        normalize(
            "# comment\nyoutube.com\n*.googlevideo.com\n+.#bad\nnot a domain\nlocalhost\nOK.Example.COM\n",
            &mut set,
        );
        assert!(set.contains("youtube.com"));
        assert!(set.contains("googlevideo.com"));
        assert!(set.contains("ok.example.com"));
        assert!(!set.iter().any(|d| d.contains(' ')));
        assert!(!set.contains("localhost"));
    }

    #[test]
    fn rebuild_merges_builtin_and_remote() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        fs::create_dir_all(paths.lists()).unwrap();
        fs::write(paths.hostlist_file(), "youtube.com\n discord.com \n").unwrap();
        fs::write(remote_list_file(&paths), "x.com\nyoutube.com\n").unwrap();
        let n = rebuild_effective(&paths).unwrap();
        assert_eq!(n, 3);
        let text = fs::read_to_string(effective_hostlist_file(&paths)).unwrap();
        assert!(text.contains("discord.com"));
        assert!(text.contains("x.com"));
        // effective file is preferred once it exists
        assert_eq!(effective_hostlist(&paths), effective_hostlist_file(&paths));
    }

    #[test]
    fn user_sources_require_https() {
        let s = Settings {
            lists_sources: "https://example.com/list.txt\nhttp://evil/list\n# comment\n".into(),
            ..Default::default()
        };
        let urls = user_sources(&s);
        assert_eq!(urls, vec!["https://example.com/list.txt"]);
    }
}
