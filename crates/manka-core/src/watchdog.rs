//! Background self-healing ("сторож"): periodically probe the active bypass
//! and report degradation, so the shell can re-run autoselection before the
//! user even notices that "вчера всё работало, а сегодня YouTube мёртв".

use serde::Serialize;

use crate::probe;
use crate::settings::Settings;

/// URLs that are never blocked and therefore must answer even when the
/// strategy is broken. If they fail too, the problem is the network itself
/// and re-picking a strategy would not help.
const CONTROL_URLS: &[&str] = &["https://www.gstatic.com/generate_204", "https://ya.ru/"];

#[derive(Serialize, Clone, Debug, Default)]
pub struct WatchdogReport {
    pub checked: bool,
    pub healthy: bool,
    /// 0..=100 — success rate over the probe targets.
    pub percent: u32,
    pub detail: String,
    pub checked_at: i64,
}

/// Probe a small target set through the running engine. Cheap by design:
/// one request per URL, short timeouts, at most 6 URLs.
pub async fn check(settings: &Settings) -> WatchdogReport {
    let targets: Vec<String> = settings.all_targets().into_iter().take(6).collect();
    if targets.is_empty() {
        return WatchdogReport {
            checked: false,
            healthy: true,
            detail: "нет целей для проверки".into(),
            checked_at: crate::util::now_ms(),
            ..Default::default()
        };
    }
    let sites = probe::check_sites(&targets, 1, 5, 6).await;
    let (percent, _) = probe::score(&sites);
    let ok: u32 = sites.iter().map(|s| s.ok).sum();
    let total: u32 = sites.iter().map(|s| s.total).sum();

    if percent >= 50 {
        return WatchdogReport {
            checked: true,
            healthy: true,
            percent,
            detail: format!("{ok}/{total} запросов успешно"),
            checked_at: crate::util::now_ms(),
        };
    }

    // Targets fail — is it the strategy or the whole network?
    let control = probe::check_sites(
        &CONTROL_URLS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        1,
        5,
        2,
    )
    .await;
    let control_ok: u32 = control.iter().map(|s| s.ok).sum();
    if control_ok == 0 {
        return WatchdogReport {
            checked: true,
            healthy: true, // не виновата стратегия — сети нет вовсе
            percent,
            detail: "цели недоступны, но и контрольные сайты не отвечают — похоже, пропал интернет"
                .into(),
            checked_at: crate::util::now_ms(),
        };
    }

    WatchdogReport {
        checked: true,
        healthy: false,
        percent,
        detail: format!(
            "доступность {percent}% ({ok}/{total}) — стратегия, похоже, перестала работать"
        ),
        checked_at: crate::util::now_ms(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_targets_report_not_checked() {
        // all_targets() of default settings is non-empty, so build one that is
        let mut s = Settings::default();
        s.targets.clear();
        s.custom_targets.clear();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let r = rt.block_on(check(&s));
        assert!(!r.checked);
    }
}
