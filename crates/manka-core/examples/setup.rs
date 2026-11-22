//! Installs the real components into the default Manka data directory and
//! activates the universal built-in strategy. Run once on a fresh machine:
//!
//!   cargo run -p manka-core --example setup
//!
//! The Tauri app will then start the engine on its next launch.

use manka_core::model::Engine;
use manka_core::paths::Paths;
use manka_core::{components, repository, store, Settings};

#[tokio::main]
async fn main() -> Result<(), String> {
    let paths = Paths::detect();
    paths.ensure().map_err(|e| e.to_string())?;
    manka_core::settings::ensure_exclude_list(&paths).map_err(|e| e.to_string())?;
    println!("data dir: {}", paths.root().display());

    let noop = |p: components::Progress| {
        if p.percent.is_multiple_of(20) {
            println!("{} {}%", p.stage, p.percent);
        }
    };

    let version = components::latest_zapret_version().await?;
    components::install_zapret(&paths, &version, &noop, false).await?;
    println!("zapret: {version}");

    let release = components::latest_tgws_release().await?;
    components::install_tgws(&paths, &release, &noop, false).await?;
    println!("tgws: {}", release.tag);

    if let Err(e) = store::refresh_index(&paths, &noop).await {
        println!("store index: {e}");
    }
    for id in ["CSYOKS001", "CSZEFL002", "CSZTR1003"] {
        if store::items(&paths).iter().any(|i| i.store_id == id) {
            match store::install_item(&paths, id, &noop).await {
                Ok(v) => println!("kit {id}: {v}"),
                Err(e) => println!("kit {id} failed: {e}"),
            }
        }
    }

    let mut settings = Settings::load(&paths);
    if let Some(s) = repository::all(&paths)
        .into_iter()
        .find(|s| s.engine == Engine::Zapret2 && s.id == "builtin-z2-faketls")
    {
        settings.set_active(Engine::Zapret2, s.id.clone());
        settings.engine = Engine::Zapret2;
        println!("active strategy: {}", s.name);
    }
    settings.enabled = true;
    settings.save(&paths).map_err(|e| e.to_string())?;
    println!("settings saved");
    Ok(())
}
