//! Manka core library.
//!
//! Everything that is not UI lives here: models, settings, component providers,
//! the strategy catalogue (built-in + CDPI kits), process runner, probes and
//! autoselection. The Tauri shell only wires commands and events to this crate.

pub mod args;
pub mod autoselect;
pub mod builtin;
pub mod components;
pub mod doctor;
pub mod history;
pub mod http;
pub mod lists;
pub mod model;
pub mod paths;
pub mod probe;
pub mod repository;
pub mod runner;
pub mod settings;
pub mod store;
pub mod util;
pub mod watchdog;

pub use model::{Engine, Source, Strategy};
pub use paths::Paths;
pub use settings::Settings;

pub const APP_NAME: &str = "Manka";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default Telegram proxy port.
pub const DEFAULT_TGWS_PORT: u16 = 1443;

/// Target groups used by autoselection and diagnostics (same set as the
/// mobile app, plus a few desktop-friendly extras).
pub const TARGET_GROUPS: &[(&str, &str, &[&str])] = &[
    (
        "youtube",
        "YouTube",
        &[
            "https://www.youtube.com/",
            "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
            "https://yt3.ggpht.com/",
            "https://redirector.googlevideo.com/report_mapping",
            "https://youtubei.googleapis.com/",
            "https://m.youtube.com/",
        ],
    ),
    (
        "discord",
        "Discord",
        &[
            "https://discord.com/",
            "https://gateway.discord.gg/",
            "https://cdn.discordapp.com/",
            "https://discordapp.com/",
            "https://media.discordapp.net/",
        ],
    ),
    (
        "general",
        "Общие сайты",
        &[
            "https://rutracker.org/",
            "https://www.instagram.com/",
            "https://www.facebook.com/",
            "https://x.com/",
            "https://www.linkedin.com/",
            "https://proton.me/",
        ],
    ),
    (
        "instagram",
        "Instagram",
        &[
            "https://www.instagram.com/",
            "https://i.instagram.com/",
            "https://static.cdninstagram.com/",
            "https://scontent.cdninstagram.com/",
            "https://graph.facebook.com/",
        ],
    ),
    (
        "facebook",
        "Facebook",
        &[
            "https://www.facebook.com/",
            "https://m.facebook.com/",
            "https://graph.facebook.com/",
            "https://b-graph.facebook.com/",
            "https://static.xx.fbcdn.net/",
        ],
    ),
    (
        "tiktok",
        "TikTok",
        &[
            "https://www.tiktok.com/",
            "https://m.tiktok.com/",
            "https://www.tiktokv.com/",
            "https://api16-normal-c-useast1a.tiktokv.com/",
            "https://p16-sign-va.tiktokcdn.com/",
        ],
    ),
    (
        "cdn",
        "Cloudflare CDN",
        &[
            "https://cdnjs.cloudflare.com/ajax/libs/jquery/3.7.1/jquery.min.js",
            "https://speed.cloudflare.com/__down?bytes=65536",
            "https://www.cloudflare.com/",
        ],
    ),
    (
        "telegram",
        "Telegram (web)",
        &[
            "https://web.telegram.org/",
            "https://telegram.org/",
            "https://t.me/",
        ],
    ),
];

pub fn group_urls(ids: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (id, _, urls) in TARGET_GROUPS {
        if ids.iter().any(|x| x == id) {
            out.extend(urls.iter().map(|u| u.to_string()));
        }
    }
    out.dedup();
    out
}
