//! Strategy repository: built-in, store kits, user and auto strategies.

use crate::model::{Engine, Source, Strategy};
use crate::paths::Paths;
use crate::settings::Settings;

pub fn user_strategies(paths: &Paths) -> Vec<Strategy> {
    crate::util::read_json(&paths.presets_file()).unwrap_or_default()
}

pub fn save_user_strategies(paths: &Paths, list: &[Strategy]) -> Result<(), String> {
    crate::util::write_json(&paths.presets_file(), &list).map_err(|e| e.to_string())
}

pub fn all(paths: &Paths) -> Vec<Strategy> {
    let mut out = crate::builtin::builtin();
    out.extend(crate::store::installed_presets(paths));
    out.extend(user_strategies(paths));
    out
}

pub fn all_with_catalog(paths: &Paths, engine: Engine, full: bool) -> Vec<Strategy> {
    let mut out = all(paths);
    out.extend(crate::builtin::catalog(engine, full));
    out
}

pub fn by_id(paths: &Paths, id: &str) -> Option<Strategy> {
    all(paths).into_iter().find(|s| s.id == id)
}

/// The strategy a profile should run: the saved one, otherwise the first
/// built-in strategy for the engine.
pub fn active(paths: &Paths, settings: &Settings, engine: Engine) -> Option<Strategy> {
    let list = all(paths);
    if let Some(id) = settings.active_strategy(engine) {
        if let Some(s) = list.iter().find(|s| &s.id == id && s.engine == engine) {
            return Some(s.clone());
        }
    }
    list.into_iter()
        .find(|s| s.engine == engine && s.source == Source::Builtin)
}

/// Store a winning autoselection result as a reusable preset.
pub fn save_auto(paths: &Paths, name: &str, engine: Engine, template: &str, percent: u32) -> Result<Strategy, String> {
    let mut list = user_strategies(paths);
    if let Some(existing) = list
        .iter()
        .find(|s| s.source == Source::Auto && s.template == template && s.engine == engine)
    {
        let mut found = existing.clone();
        found.score = Some(percent);
        if found.name.trim_start().starts_with('-') {
            found.name = auto_display_name(name);
        }
        let _ = save_user(paths, found.clone());
        return Ok(found);
    }
    let strategy = auto_strategy(engine, name, template, percent);
    list.push(strategy.clone());
    save_user_strategies(paths, &list)?;
    Ok(strategy)
}

fn auto_display_name(name: &str) -> String {
    if name.trim_start().starts_with('-') {
        let short: String = name
            .trim_start_matches('-')
            .replace("lua-desync=", "")
            .replace([':', '='], " ")
            .chars()
            .take(48)
            .collect();
        format!(
            "Авто · {} · {short}",
            chrono::Local::now().format("%d.%m %H:%M")
        )
    } else {
        name.to_string()
    }
}

fn auto_strategy(engine: Engine, name: &str, template: &str, percent: u32) -> Strategy {
    let id = format!("auto-{}", crate::util::hash_id(template));
    let mut s = Strategy::new(id, engine, auto_display_name(name), template);
    s.source = Source::Auto;
    s.score = Some(percent);
    s.created_at = crate::util::now_ms();
    s
}

/// Save/update a batch of autoselection results in one write. Used at the end
/// of a run so every successful strategy is visible on the Strategies screen.
pub fn save_auto_results(
    paths: &Paths,
    items: &[(Engine, String, String, u32)],
) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    let mut list = user_strategies(paths);
    for (engine, name, template, percent) in items {
        if let Some(existing) = list.iter_mut().find(|s| {
            s.source == Source::Auto && s.engine == *engine && s.template == *template
        }) {
            existing.score = Some(*percent);
            continue;
        }
        list.push(auto_strategy(*engine, name, template, *percent));
    }
    save_user_strategies(paths, &list)
}

pub fn save_user(paths: &Paths, strategy: Strategy) -> Result<(), String> {
    let mut list = user_strategies(paths);
    list.retain(|s| s.id != strategy.id);
    list.push(strategy);
    save_user_strategies(paths, &list)
}

pub fn delete_user(paths: &Paths, id: &str) -> Result<(), String> {
    let mut list = user_strategies(paths);
    list.retain(|s| s.id != id);
    save_user_strategies(paths, &list)
}

/// Remember which strategy each engine uses.
pub fn set_active(paths: &Paths, settings: &mut Settings, engine: Engine, id: &str) -> Result<(), String> {
    settings.set_active(engine, id);
    settings.engine = engine;
    settings.save(paths).map_err(|e| e.to_string())
}
