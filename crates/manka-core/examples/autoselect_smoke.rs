//! Live autoselection smoke test (runs winws2 for real, needs administrator):
//!
//!   cargo run -p manka-core --example autoselect_smoke
//!
//! Uses the current settings and the first few catalogue candidates.

use std::sync::atomic::AtomicBool;

use manka_core::autoselect::{self, AutoRequest};
use manka_core::paths::Paths;
use manka_core::runner::ProcessManager;
use manka_core::{builtin, repository, Settings};

#[tokio::main]
async fn main() -> Result<(), String> {
    let paths = Paths::detect();
    let settings = Settings::load(&paths);
    let engine = settings.engine;
    if !manka_core::util::is_elevated() {
        return Err("run as administrator".into());
    }

    let mut strategies = repository::all(&paths);
    strategies.extend(builtin::catalog(engine, false).into_iter().take(4));

    let processes = ProcessManager::default();
    let cancel = AtomicBool::new(false);
    let request = AutoRequest {
        engines: vec![engine],
        targets: vec![
            "https://www.youtube.com/".into(),
            "https://discord.com/".into(),
        ],
        full: false,
        include_store: false,
        requests: 1,
        timeout_sec: 6,
        stop_after_perfect: 1,
    };

    let state = autoselect::run(
        &paths,
        &settings,
        &strategies,
        &processes,
        &request,
        &cancel,
        |s| {
            if s.phase == "testing" || s.phase == "done" {
                println!("{}/{} {} — {}", s.current, s.total, s.current_name, s.phase);
            }
        },
    )
    .await;

    println!("phase: {}", state.phase);
    if let Some(best) = state.best() {
        println!("best: {}% {} ({} ms)", best.percent, best.name, best.avg_ms);
    } else {
        println!("no working strategy found (baseline may already be 100%)");
    }
    for r in state.sorted() {
        println!(
            "  {:>3}% {:>5}ms {} {}",
            r.percent,
            r.avg_ms,
            r.engine.id(),
            r.name
        );
    }
    Ok(())
}
