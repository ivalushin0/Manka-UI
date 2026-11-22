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
        if existing.name.trim_start().starts_with('-') {
            let short: String = name
                .trim_start_matches('-')
                .replace("lua-desync=", "")
                .replace([':', '='], " ")
                .chars()
                .take(48)
                .collect();
            let new_name = format!(
                "Авто · {} · {short}",
                chrono::Local::now().format("%d.%m %H:%M")
            );
            let mut fixed = existing.clone();
            fixed.name = new_name;
            let _ = save_user(paths, fixed.clone());
            return Ok(fixed);
        }
        return Ok(existing.clone());
    }
    let display_name = if name.trim_start().starts_with('-') {
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
    };
    let id = format!("auto-{}", crate::util::hash_id(template));
    let mut s = Strategy::new(id, engine, display_name, template);
    s.source = Source::Auto;
    s.score = Some(percent);
    s.created_at = crate::util::now_ms();
    list.push(s.clone());
    save_user_strategies(paths, &list)?;
    Ok(s)
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
