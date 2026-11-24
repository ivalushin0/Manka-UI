//! Strategy autoselection: try candidates one by one, probe the same target
//! set through each, rank by success rate and latency.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::history::{HistoryRun, SavedResult};
use crate::model::{Engine, SiteResult, Source, Strategy};
use crate::paths::Paths;
use crate::probe;
use crate::runner::ProcessManager;
use crate::settings::Settings;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AutoRequest {
    pub engines: Vec<Engine>,
    pub targets: Vec<String>,
    pub full: bool,
    pub include_store: bool,
    pub requests: u32,
    pub timeout_sec: u32,
    pub stop_after_perfect: u32,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct CandidateResult {
    pub strategy_id: String,
    pub name: String,
    pub engine: Engine,
    pub source: Source,
    pub template: String,
    pub percent: u32,
    pub ok: u32,
    pub total: u32,
    pub avg_ms: u64,
    pub sites: Vec<SiteResult>,
    pub start_error: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct AutoState {
    pub phase: String,
    pub engines: Vec<Engine>,
    pub current: u32,
    pub total: u32,
    pub current_name: String,
    pub current_engine: Option<Engine>,
    pub targets: Vec<String>,
    pub baseline: Vec<SiteResult>,
    pub results: Vec<CandidateResult>,
    pub error: Option<String>,
}

impl AutoState {
    pub fn sorted(&self) -> Vec<&CandidateResult> {
        let mut v: Vec<&CandidateResult> = self.results.iter().collect();
        v.sort_by(|a, b| {
            b.percent
                .cmp(&a.percent)
                .then_with(|| {
                    let am = if a.avg_ms == 0 { u64::MAX } else { a.avg_ms };
                    let bm = if b.avg_ms == 0 { u64::MAX } else { b.avg_ms };
                    am.cmp(&bm)
                })
        });
        v
    }

    pub fn best(&self) -> Option<&CandidateResult> {
        self.sorted().into_iter().find(|r| r.percent > 0)
    }

    pub fn best_by_engine(&self) -> Vec<CandidateResult> {
        let mut out: Vec<CandidateResult> = Vec::new();
        for engine in &self.engines {
            if let Some(best) = self
                .sorted()
                .into_iter()
                .find(|r| r.percent > 0 && r.engine == *engine)
            {
                out.push(best.clone());
            }
        }
        out
    }
}

fn candidates(strategies: &[Strategy], settings: &Settings, request: &AutoRequest) -> Vec<Strategy> {
    let mut out: Vec<Strategy> = Vec::new();
    for engine in &request.engines {
        let active = settings.active_strategy(*engine).and_then(|id| {
            strategies
                .iter()
                .find(|s| &s.id == id && s.engine == *engine)
                .cloned()
        });
        let current = active.or_else(|| {
            strategies
                .iter()
                .find(|s| s.engine == *engine && s.source == Source::Builtin)
                .cloned()
        });
        if let Some(c) = current {
            out.push(c);
        }
        if request.include_store {
            let store_limit = if request.full { usize::MAX } else { 3 };
            out.extend(
                strategies
                    .iter()
                    .filter(|s| s.engine == *engine && s.source == Source::Store)
                    .take(store_limit)
                    .cloned(),
            );
        }
        let catalog_limit = if request.full { usize::MAX } else { 6 };
        out.extend(
            crate::builtin::catalog(*engine, request.full)
                .into_iter()
                .take(catalog_limit),
        );
    }
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|s| seen.insert(format!("{}::{}", s.engine.id(), s.template)));
    out
}

fn render_args(
    paths: &Paths,
    settings: &Settings,
    strategy: &Strategy,
) -> Result<crate::args::Rendered, String> {
    let kit = strategy.kit_id.as_ref().map(|id| paths.kit(id));
    let engine_dir = paths.engine_dir();
    crate::args::render(strategy, settings, paths, engine_dir.as_deref(), kit.as_deref())
}

fn tail(path: &std::path::Path, n: usize) -> String {
    crate::util::tail_lines(path, n).join(" | ")
}

pub async fn run<F>(
    paths: &Paths,
    settings: &Settings,
    strategies: &[Strategy],
    processes: &ProcessManager,
    request: &AutoRequest,
    cancel: &AtomicBool,
    progress: F,
) -> AutoState
where
    F: Fn(&AutoState) + Send + Sync,
{
    let engine_log = paths.logs().join("engine.log");
    let list = candidates(strategies, settings, request);
    let mut state = AutoState {
        phase: "baseline".into(),
        engines: request.engines.clone(),
        total: list.len() as u32,
        targets: request.targets.clone(),
        ..Default::default()
    };
    progress(&state);

    if list.is_empty() {
        state.phase = "error".into();
        state.error = Some("Нет стратегий для проверки".into());
        progress(&state);
        return state;
    }
    if request.targets.is_empty() {
        state.phase = "error".into();
        state.error = Some("Не выбраны цели проверки".into());
        progress(&state);
        return state;
    }

    let _ = processes.stop_engine(&engine_log).await;
    crate::util::trim_log(&engine_log, 2 * 1024 * 1024);

    state.baseline = probe::check_sites(
        &request.targets,
        request.requests.max(1),
        request.timeout_sec.max(2),
        8,
    )
    .await;
    state.phase = "testing".into();
    progress(&state);

    let mut perfect: std::collections::BTreeMap<Engine, u32> = std::collections::BTreeMap::new();
    let mut start_failures: std::collections::BTreeMap<Engine, u32> = std::collections::BTreeMap::new();

    for (idx, candidate) in list.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            state.phase = "cancelled".into();
            break;
        }
        if let Some(limit) = Some(request.stop_after_perfect).filter(|n| *n > 0) {
            if perfect.get(&candidate.engine).copied().unwrap_or(0) >= limit {
                continue;
            }
        }
        state.current = idx as u32 + 1;
        state.current_name = candidate.name.clone();
        state.current_engine = Some(candidate.engine);
        progress(&state);

        let rendered = match render_args(paths, settings, candidate) {
            Ok(r) => r,
            Err(e) => {
                state.results.push(CandidateResult {
                    strategy_id: candidate.id.clone(),
                    name: candidate.name.clone(),
                    engine: candidate.engine,
                    source: candidate.source,
                    template: candidate.template.clone(),
                    start_error: Some(e),
                    ..Default::default()
                });
                progress(&state);
                continue;
            }
        };
        let Some(exe) = rendered.exe.clone() else {
            state.results.push(CandidateResult {
                strategy_id: candidate.id.clone(),
                name: candidate.name.clone(),
                engine: candidate.engine,
                source: candidate.source,
                template: candidate.template.clone(),
                start_error: Some("Zapret не установлен — откройте «Компоненты»".into()),
                ..Default::default()
            });
            progress(&state);
            continue;
        };

        let started = Instant::now();
        let start = processes
            .start_engine(&exe, &rendered.args, rendered.working_dir.as_deref(), &engine_log)
            .await;
        if let Err(e) = start {
            state.results.push(CandidateResult {
                strategy_id: candidate.id.clone(),
                name: candidate.name.clone(),
                engine: candidate.engine,
                source: candidate.source,
                template: candidate.template.clone(),
                start_error: Some(e),
                ..Default::default()
            });
            progress(&state);
            continue;
        }

        let settle = if candidate.source == Source::Store { 1600 } else { 800 };
        tokio::time::sleep(Duration::from_millis(settle)).await;
        let status = processes.engine_status(&engine_log).await;
        if !status.running {
            let detail = tail(&engine_log, 8);
            processes.stop_engine(&engine_log).await;
            *start_failures.entry(candidate.engine).or_insert(0) += 1;
            state.results.push(CandidateResult {
                strategy_id: candidate.id.clone(),
                name: candidate.name.clone(),
                engine: candidate.engine,
                source: candidate.source,
                template: candidate.template.clone(),
                start_error: Some(if detail.is_empty() {
                    "движок не запустился".into()
                } else {
                    format!("движок не запустился: {detail}")
                }),
                ..Default::default()
            });
            progress(&state);
            continue;
        }

        let sites = probe::check_sites(
            &request.targets,
            request.requests.max(1),
            request.timeout_sec.max(2),
            8,
        )
        .await;
        let (percent, avg_ms) = probe::score(&sites);
        let ok: u32 = sites.iter().map(|s| s.ok).sum();
        let total: u32 = sites.iter().map(|s| s.total).sum();
        let _elapsed = started.elapsed().as_secs();
        processes.stop_engine(&engine_log).await;
        if total > 0 && ok == total {
            *perfect.entry(candidate.engine).or_insert(0) += 1;
        }
        state.results.push(CandidateResult {
            strategy_id: candidate.id.clone(),
            name: candidate.name.clone(),
            engine: candidate.engine,
            source: candidate.source,
            template: candidate.template.clone(),
            percent,
            ok,
            total,
            avg_ms,
            sites,
            start_error: None,
        });
        progress(&state);
    }

    if state.phase != "cancelled" {
        state.phase = "done".into();
    }
    state.current_engine = None;
    progress(&state);

    // Persist successful autoselection results as reusable "auto" presets so
    // they show up on the Strategies screen with their availability score.
    let auto_items: Vec<(Engine, String, String, u32)> = state
        .sorted()
        .into_iter()
        .filter(|r| r.percent > 0 && r.start_error.is_none())
        .map(|r| (r.engine, r.name.clone(), r.template.clone(), r.percent))
        .collect();
    let _ = crate::repository::save_auto_results(paths, &auto_items);

    // history: keep every tested strategy (including failures) so the UI can
    // show the complete picture of a run.
    let results: Vec<SavedResult> = state
        .sorted()
        .into_iter()
        .map(|r| SavedResult {
            name: r.name.clone(),
            template: r.template.clone(),
            strategy_id: r.strategy_id.clone(),
            engine: r.engine.id().to_string(),
            percent: r.percent,
            ok: r.ok,
            total: r.total,
            avg_ms: r.avg_ms,
            sites: r.sites.clone(),
            start_error: r.start_error.clone(),
        })
        .collect();
    if !results.is_empty() {
        crate::history::append(
            paths,
            HistoryRun {
                id: crate::util::now_ms().to_string(),
                time: crate::util::now_ms(),
                engine: request.engines.first().map(|e| e.id().to_string()),
                targets: request.targets.clone(),
                baseline_ok: state.baseline.iter().map(|s| s.ok).sum(),
                baseline_total: state.baseline.iter().map(|s| s.total).sum(),
                baseline: state.baseline.clone(),
                tested: state.results.len() as u32,
                results,
            },
        );
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn best_ignores_zero() {
        let s = AutoState {
            results: vec![
                CandidateResult {
                    percent: 0,
                    avg_ms: 10,
                    ..Default::default()
                },
                CandidateResult {
                    percent: 80,
                    avg_ms: 500,
                    ..Default::default()
                },
                CandidateResult {
                    percent: 80,
                    avg_ms: 200,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(s.best().unwrap().avg_ms, 200);
    }
}
