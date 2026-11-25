//! End-to-end smoke test against real upstream components.
//!
//! Run with:
//!   cargo test -p manka-core --test smoke -- --ignored --nocapture
//!
//! It downloads the Zapret bundle, the Telegram proxy and one CDPI kit, renders
//! strategies and (when elevated) starts winws2 for a couple of seconds.

use std::path::PathBuf;

use manka_core::model::{Engine, Source};
use manka_core::paths::Paths;
use manka_core::runner::ProcessManager;
use manka_core::settings::Settings;
use manka_core::{args, builtin, components, repository, store};

fn check(args: &[String], engine_dir: &std::path::Path) -> Result<(), String> {
    for a in args {
        for prefix in [
            "--hostlist=",
            "--hostlist-exclude=",
            "--ipset=",
            "--ipset-exclude=",
            "--hostlist-domains=",
        ] {
            if let Some(path) = a.strip_prefix(prefix) {
                if (path.starts_with('/') || path.contains(":\\"))
                    && !std::path::Path::new(path).exists()
                {
                    return Err(format!("missing referenced list: {path}"));
                }
            }
        }
        if let Some(rest) = a.strip_prefix("--blob=") {
            if let Some(path) = rest.split_once('@').map(|(_, p)| p) {
                if (path.contains(":\\") || path.starts_with('/'))
                    && !std::path::Path::new(path).exists()
                {
                    return Err(format!("missing blob: {path}"));
                }
            }
        }
        if let Some(rest) = a.strip_prefix("--lua-init=@") {
            if !std::path::Path::new(rest).exists() {
                return Err(format!("missing lua init: {rest}"));
            }
        }
    }
    if args.iter().any(|a| a.contains('%')) {
        return Err(format!("unresolved variable in {args:?}"));
    }
    if !args.iter().any(|a| a.starts_with("--wf-tcp")) {
        return Err("no WinDivert tcp filter".into());
    }
    let _ = engine_dir;
    Ok(())
}

#[tokio::test]
#[ignore]
async fn installs_and_renders() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    paths.ensure().unwrap();
    manka_core::settings::ensure_exclude_list(&paths).unwrap();
    let settings = Settings::default();

    // ---- Zapret bundle
    let version = components::latest_zapret_version().await.unwrap();
    println!("zapret latest: {version}");
    let noop = |_p: components::Progress| {};
    components::install_zapret(&paths, &version, &noop, true)
        .await
        .unwrap();
    let engine_dir = paths.engine_dir().expect("engine dir");
    assert!(engine_dir.join("winws2.exe").exists());
    assert!(engine_dir.join("winws.exe").exists());
    println!("engine dir: {}", engine_dir.display());

    // ---- built-in strategy rendering
    let builtins = builtin::builtin();
    let z2 = builtins
        .iter()
        .find(|s| s.engine == Engine::Zapret2)
        .unwrap();
    let rendered = args::render(z2, &settings, &paths, Some(&engine_dir), None).unwrap();
    println!("builtin args: {}", rendered.command_line);
    check(&rendered.args, &engine_dir).unwrap();
    assert!(rendered.exe.is_some());

    // ---- Telegram proxy
    let release = components::latest_tgws_release().await.unwrap();
    println!("tgws release: {} {}", release.tag, release.asset);
    components::install_tgws(&paths, &release, &noop, true)
        .await
        .unwrap();
    let exe = paths.tgws_exe().expect("tgws exe");
    println!("tgws exe: {}", exe.display());
    let out = std::process::Command::new(&exe)
        .arg("--version")
        .output()
        .unwrap();
    println!(
        "tgws version: {}",
        String::from_utf8_lossy(&out.stdout).trim()
    );
    assert!(out.status.success());

    // ---- store index + kit
    store::refresh_index(&paths, &noop).await.unwrap();
    let items = store::items(&paths);
    println!("store items: {}", items.len());
    assert!(items.len() >= 2);
    let item = items
        .iter()
        .find(|i| i.store_id == "CSYOKS001")
        .expect("YtDisBystro kit");
    let kit_version = store::install_item(&paths, &item.store_id, &noop)
        .await
        .unwrap();
    println!("kit {} installed: {kit_version}", item.store_id);

    let presets = store::installed_presets(&paths);
    println!("kit presets: {}", presets.len());
    assert!(!presets.is_empty(), "no presets parsed from kit");

    let mut rendered_any = 0;
    for preset in &presets {
        let kit_dir = preset.kit_id.as_ref().map(|id| paths.kit(id));
        match args::render(
            preset,
            &settings,
            &paths,
            Some(&engine_dir),
            kit_dir.as_deref(),
        ) {
            Ok(r) => match check(&r.args, &engine_dir) {
                Ok(()) => {
                    println!("rendered {} ({} args)", preset.name, r.args.len());
                    rendered_any += 1;
                }
                Err(e) => println!("skip {}: {e}", preset.name),
            },
            Err(e) => println!("skip {}: {e}", preset.name),
        }
    }
    assert!(rendered_any > 0);
    assert!(presets.iter().any(|p| p.source == Source::Store));

    // ---- active strategy lookup
    let active = repository::active(&paths, &settings, Engine::Zapret2).expect("active strategy");
    println!("active: {}", active.name);

    // ---- actually start winws2 (needs administrator)
    if manka_core::util::is_elevated() {
        let pm = ProcessManager::default();
        let log = paths.logs().join("engine.log");
        let rendered = args::render(
            &active,
            &settings,
            &paths,
            Some(&engine_dir),
            active.kit_id.as_ref().map(|id| paths.kit(id)).as_deref(),
        )
        .unwrap();
        let exe = rendered.exe.clone().unwrap();
        pm.start_engine(&exe, &rendered.args, rendered.working_dir.as_deref(), &log)
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let status = pm.engine_status(&log).await;
        let tail = manka_core::util::tail_lines(&log, 20).join("\n");
        pm.stop_engine(&log).await;
        println!(
            "engine running after 3s: {} {}\n{tail}",
            status.running,
            status.detail.unwrap_or_default()
        );
        assert!(status.running, "winws2 did not stay running:\n{tail}");
    } else {
        println!("not elevated, skipping live winws2 start");
    }

    // keep the temp dir path in the output for inspection
    println!("data dir: {:?}", PathBuf::from(dir.path()));
}
