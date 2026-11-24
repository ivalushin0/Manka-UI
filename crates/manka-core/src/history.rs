//! Autoselection history.

use serde::{Deserialize, Serialize};

use crate::model::SiteResult;
use crate::paths::Paths;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SavedResult {
    pub name: String,
    pub template: String,
    pub strategy_id: String,
    pub engine: String,
    pub percent: u32,
    pub ok: u32,
    pub total: u32,
    pub avg_ms: u64,
    #[serde(default)]
    pub sites: Vec<SiteResult>,
    #[serde(default)]
    pub start_error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct HistoryRun {
    pub id: String,
    pub time: i64,
    pub engine: Option<String>,
    pub targets: Vec<String>,
    pub baseline_ok: u32,
    pub baseline_total: u32,
    #[serde(default)]
    pub baseline: Vec<SiteResult>,
    pub tested: u32,
    pub results: Vec<SavedResult>,
}

pub fn load(paths: &Paths) -> Vec<HistoryRun> {
    crate::util::read_json(&paths.history_file()).unwrap_or_default()
}

pub fn append(paths: &Paths, run: HistoryRun) {
    let mut runs = load(paths);
    runs.insert(0, run);
    runs.truncate(30);
    let _ = crate::util::write_json(&paths.history_file(), &runs);
}
