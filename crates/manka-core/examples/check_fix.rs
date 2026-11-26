//! Diagnostic for the 0.4.1 list fixes. Uses the real data directory:
//! rebuilds the effective hostlist, checks the critical domains and renders
//! the active strategy to verify the QUIC drop is hostlist-scoped.
//!
//!   cargo run -p manka-core --example check_fix

use manka_core::paths::Paths;
use manka_core::{args, lists, repository, settings, Settings};

fn main() -> Result<(), String> {
    let paths = Paths::detect();
    settings::ensure_exclude_list(&paths).map_err(|e| e.to_string())?;
    settings::ensure_hostlist(&paths).map_err(|e| e.to_string())?;
    let count = lists::ensure_defaults(&paths).map_err(|e| e.to_string())?;

    let hostlist = lists::effective_hostlist(&paths);
    let text = std::fs::read_to_string(&hostlist).map_err(|e| e.to_string())?;
    println!("effective hostlist: {hostlist:?} ({count} entries)");
    for domain in ["pclead.co.uk", "nothingthere.co.uk", "youtube.com", "^dns.google"] {
        println!("  {domain}: {}", text.lines().any(|l| l.trim() == domain));
    }
    let exclude = std::fs::read_to_string(paths.exclude_list()).unwrap_or_default();
    for domain in ["accounts.google.com", "www.gstatic.com", "ssl.gstatic.com", "fonts.googleapis.com"] {
        println!("  exclude {domain}: {}", exclude.lines().any(|l| l.trim() == domain));
    }

    let settings = Settings::load(&paths);
    let strategy = repository::active(&paths, &settings, settings.engine)
        .ok_or("no active strategy")?;
    let kit = strategy.kit_id.as_ref().map(|id| paths.kit(id));
    let rendered = args::render(
        &strategy,
        &settings,
        &paths,
        paths.engine_dir().as_deref(),
        kit.as_deref(),
    )?;
    println!("active: {}", strategy.name);
    println!("args: {}", rendered.command_line);
    let quic = rendered
        .args
        .iter()
        .position(|a| a == "--filter-l7=quic")
        .ok_or("no quic profile")?;
    let start = quic.saturating_sub(3);
    let end = (quic + 12).min(rendered.args.len());
    println!("quic profile: {:?}", &rendered.args[start..end]);
    let has_hostlist = rendered.args[..end]
        .iter()
        .rev()
        .skip_while(|a| **a != "--new")
        .any(|a| a.starts_with("--hostlist=") || a.starts_with("--hostlist-domains="));
    println!("quic hostlist-scoped: {has_hostlist}");
    Ok(())
}
