//! CDPIUI-Store catalogue and config kits.
//!
//! Kits are the same archives CDPI UI installs: JSON presets (`IC:`/`UC:`) plus
//! `Bin/`, `List/` and Lua files. On Windows they run directly through
//! `winws.exe`/`winws2.exe`, so no conversion is needed.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::{json, Value};

use crate::args;
use crate::components::extract_zip;
use crate::http::{download, get_json, ProgressFn};
use crate::model::{Engine, Source, StoreItem, Strategy, StrategyFlag, StrategyOption};
use crate::paths::Paths;
use crate::util::{hash_id, read_json, verify_sha256, write_json};

const STORE_ZIP: &str = "https://codeload.github.com/Storik4pro/CDPIUI-Store/zip/refs/heads/main";

fn read_index(paths: &Paths, path: &str) -> Option<String> {
    let clean = path.trim().trim_start_matches('/');
    let file = paths.store_index().join(clean);
    fs::read_to_string(file).ok()
}

fn json_of(text: &str) -> Option<Value> {
    serde_json::from_str(text).ok()
}

/// `$LOADDYNAMIC(path)` fields of the store index.
fn dynamic(paths: &Paths, value: &str, lang: &str) -> String {
    let re = Regex::new(r"^\$LOADDYNAMIC\((.*)\)$").unwrap();
    let trimmed = value.trim();
    let Some(caps) = re.captures(trimmed) else {
        return value.to_string();
    };
    let rel = caps[1].trim();
    for code in [lang, "RU", "EN"] {
        let p = rel.replace("{LOC_CODE}", code);
        if let Some(text) = read_index(paths, &p) {
            return text;
        }
    }
    String::new()
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string()
}

/// Download the store index (only JSON and markdown) and replace the local copy.
pub async fn refresh_index(paths: &Paths, progress: ProgressFn<'_>) -> Result<(), String> {
    let temp = paths.temp();
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let zip_path = temp.join("cdpi-store.zip");
    download(
        STORE_ZIP,
        &zip_path,
        progress,
        "store",
        "Обновление магазина",
    )
    .await?;

    let unpack = temp.join("cdpi-store");
    if unpack.exists() {
        let _ = fs::remove_dir_all(&unpack);
    }
    extract_zip(&zip_path, &unpack, None)?;
    let top = fs::read_dir(&unpack)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir())
        .ok_or("Неожиданная структура архива магазина")?;

    let target = paths.store_index();
    if target.exists() {
        let _ = fs::remove_dir_all(&target);
    }
    fs::rename(&top, &target).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&unpack);
    let _ = fs::remove_file(&zip_path);
    Ok(())
}

pub fn items(paths: &Paths) -> Vec<StoreItem> {
    let Some(root_text) = read_index(paths, "init.json") else {
        return Vec::new();
    };
    let Some(root) = json_of(&root_text) else {
        return Vec::new();
    };
    let Some(dirs) = root.get("categories_directory").and_then(|v| v.as_object()) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for (category, dir_val) in dirs {
        let Some(dir) = dir_val.as_str() else {
            continue;
        };
        let Some(cat_text) = read_index(paths, &format!("{dir}/init.json")) else {
            continue;
        };
        let Some(cat) = json_of(&cat_text) else {
            continue;
        };
        let Some(item_dirs) = cat.get("items_directories").and_then(|v| v.as_object()) else {
            continue;
        };
        let order: Vec<String> = cat
            .get("items")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_else(|| item_dirs.keys().cloned().collect());

        for key in order {
            let Some(sub) = item_dirs.get(&key).and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(text) = read_index(paths, &format!("{dir}/{sub}/init.json")) else {
                continue;
            };
            let Some(obj) = json_of(&text) else { continue };
            let Some(store_id) = obj.get("store_id").and_then(|v| v.as_str()) else {
                continue;
            };
            let kind = str_field(&obj, "type");
            let small = dynamic(paths, &str_field(&obj, "small_description"), "RU");
            let description = dynamic(paths, &str_field(&obj, "description"), "RU");
            let warning = if str_field(&obj, "display_warning") == "true" {
                let w = dynamic(paths, &str_field(&obj, "warning_text"), "RU");
                (!w.trim().is_empty()).then_some(w.trim().to_string())
            } else {
                None
            };
            out.push(StoreItem {
                store_id: store_id.to_string(),
                category: category.clone(),
                kind,
                name: str_field(&obj, "name"),
                short_name: {
                    let s = str_field(&obj, "short_name");
                    if s.is_empty() {
                        str_field(&obj, "name")
                    } else {
                        s
                    }
                },
                developer: str_field(&obj, "developer"),
                description: if !small.trim().is_empty() {
                    format!("{}\n\n{}", small.trim(), description.trim())
                } else {
                    description.trim().to_string()
                },
                warning,
                version_control: str_field(&obj, "version_control"),
                link: str_field(&obj, "version_control_link"),
                preferred_file: obj
                    .get("preffered_to_download_file_name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                installed_version: None,
            });
        }
    }
    out
}

fn installed_kits(paths: &Paths) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(entries) = fs::read_dir(paths.kits()) else {
        return out;
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let meta = entry.path().join("manka-kit.json");
        if let Some(v) = read_json::<Value>(&meta) {
            let version = v
                .get("version")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            out.insert(entry.file_name().to_string_lossy().to_string(), version);
        }
    }
    out
}

pub fn items_with_installed(paths: &Paths) -> Vec<StoreItem> {
    let installed = installed_kits(paths);
    let mut items = items(paths);
    for item in &mut items {
        item.installed_version = installed.get(&item.store_id).cloned();
        if item.installed_version.is_none() && item.kind == "lsubscription" {
            let f = paths.lists().join(format!("{}.txt", item.store_id));
            if f.exists() {
                item.installed_version = Some("installed".into());
            }
        }
    }
    items
}

#[derive(Debug, Clone)]
pub struct Release {
    pub tag: String,
    pub url: String,
    pub name: String,
    /// SHA256 digest of the asset as published by GitHub.
    pub digest: Option<String>,
}

fn repo_of(link: &str) -> String {
    link.trim()
        .trim_start_matches("https://github.com/")
        .trim_end_matches('/')
        .to_string()
}

pub async fn latest_release(item: &StoreItem) -> Result<Release, String> {
    if item.version_control == "subscription" {
        return Ok(Release {
            tag: "latest".into(),
            url: item.link.clone(),
            name: item
                .link
                .rsplit('/')
                .next()
                .unwrap_or("list.txt")
                .to_string(),
            digest: None,
        });
    }
    let repo = repo_of(&item.link);
    let json = get_json(&format!(
        "https://api.github.com/repos/{repo}/releases/latest"
    ))
    .await?;
    let tag = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("В {repo} нет релизов"))?
        .to_string();
    let empty = Vec::new();
    let assets = json
        .get("assets")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    let mut choices: Vec<(String, String, Option<String>)> = assets
        .iter()
        .filter_map(|a| {
            Some((
                a.get("name")?.as_str()?.to_string(),
                a.get("browser_download_url")?.as_str()?.to_string(),
                a.get("digest").and_then(|d| d.as_str()).map(str::to_string),
            ))
        })
        .collect();
    if let Some(pref) = &item.preferred_file {
        assets_ensure_preferred(&mut choices, pref);
    }
    let pick = choices
        .iter()
        .find(|(name, _, _)| name.ends_with(".zip"))
        .or_else(|| choices.first())
        .ok_or_else(|| format!("В релизе {repo} {tag} нет файлов"))?;
    Ok(Release {
        tag,
        url: pick.1.clone(),
        name: pick.0.clone(),
        digest: pick.2.clone(),
    })
}

fn assets_ensure_preferred(choices: &mut [(String, String, Option<String>)], preferred: &str) {
    choices.sort_by_key(|(name, _, _)| !(name == preferred || name.starts_with(preferred)));
}

pub async fn install_item(
    paths: &Paths,
    store_id: &str,
    progress: ProgressFn<'_>,
) -> Result<String, String> {
    let item = items(paths)
        .into_iter()
        .find(|i| i.store_id == store_id)
        .ok_or_else(|| format!("Нет элемента магазина {store_id}"))?;

    if item.kind == "component" {
        return Err("Компоненты ставятся на вкладке «Компоненты»".into());
    }
    if item.kind != "configlist" && item.kind != "lsubscription" {
        return Err(format!("Тип {} не поддерживается", item.kind));
    }

    let release = latest_release(&item).await?;
    let temp = paths.temp();
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let ext = Path::new(&release.name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let file = temp.join(format!("kit-{store_id}.{ext}"));
    download(&release.url, &file, progress, store_id, "Скачивание набора").await?;
    // Kit archives contain Lua that winws runs with administrator rights, so
    // verify them against the digest GitHub published for the asset.
    if let Some(digest) = &release.digest {
        verify_sha256(&file, digest)?;
    } else if ext.eq_ignore_ascii_case("zip") {
        let _ = fs::remove_file(&file);
        return Err("GitHub не вернул SHA256 для архива набора — установка отменена".into());
    }

    if item.kind == "lsubscription" {
        fs::create_dir_all(paths.lists()).map_err(|e| e.to_string())?;
        let dest = paths.lists().join(format!("{store_id}.txt"));
        if file.extension().and_then(|e| e.to_str()) == Some("zip") {
            let dir = temp.join(format!("list-{store_id}"));
            extract_zip(&file, &dir, None)?;
            let first = fs::read_dir(&dir)
                .map_err(|e| e.to_string())?
                .flatten()
                .map(|e| e.path())
                .find(|p| p.is_file())
                .ok_or("Пустой архив списка")?;
            fs::copy(&first, &dest).map_err(|e| e.to_string())?;
            let _ = fs::remove_dir_all(&dir);
        } else {
            fs::copy(&file, &dest).map_err(|e| e.to_string())?;
        }
        let _ = fs::remove_file(&file);
        return Ok(release.tag);
    }

    let dest = paths.kit(store_id);
    if dest.exists() {
        let _ = fs::remove_dir_all(&dest);
    }
    extract_zip(&file, &dest, None)?;
    let meta = json!({
        "name": item.short_name,
        "version": release.tag,
        "store_id": store_id,
        "source": item.link,
        "installed_at": crate::util::now_ms(),
    });
    write_json(&dest.join("manka-kit.json"), &meta).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&file);
    Ok(release.tag)
}

/// Store ids look like `CSZTR1003` / `ASGKOI001`: short ASCII alphanumerics.
/// Anything else (path separators, dots, unicode) is rejected before it can
/// reach the file system — this runs with administrator rights.
pub fn valid_store_id(store_id: &str) -> bool {
    (4..=32).contains(&store_id.len())
        && store_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn remove_kit(paths: &Paths, store_id: &str) -> Result<(), String> {
    if !valid_store_id(store_id) {
        return Err(format!("Некорректный идентификатор набора: {store_id:?}"));
    }
    let kits_root = paths.kits();
    let dir = paths.kit(store_id);
    // defense in depth: the resolved path must stay inside the kits folder
    if !dir.starts_with(&kits_root) {
        return Err("Некорректный путь набора".into());
    }
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let list = paths.lists().join(format!("{store_id}.txt"));
    if list.exists() {
        let _ = fs::remove_file(list);
    }
    Ok(())
}

// ---------------------------------------------------------------- kit presets

fn load_strings(kit: &Path) -> (BTreeMap<String, String>, String) {
    let mut strings = BTreeMap::new();
    let Some(init) = read_json::<Value>(&kit.join("init.json")) else {
        return (strings, "RU".into());
    };
    let dirs = init
        .get("localized_strings_directory")
        .and_then(|v| v.as_object());
    let code = dirs
        .and_then(|d| d.get("RU").and_then(|v| v.as_str()).map(|_| "RU"))
        .unwrap_or("EN");
    let rel = dirs
        .and_then(|d| d.get(code))
        .and_then(|v| v.as_str())
        .unwrap_or("loc/RU/strings.json");
    if let Some(obj) = read_json::<Value>(&kit.join(rel)) {
        if let Some(map) = obj.as_object() {
            for (k, v) in map {
                if let Some(s) = v.as_str() {
                    strings.insert(k.clone(), s.to_string());
                }
            }
        }
    }
    (strings, code.to_string())
}

fn localize(text: &str, strings: &BTreeMap<String, String>) -> String {
    let re = Regex::new(r"\$LOADSTRING\(([^)]*)\)").unwrap();
    let out = re.replace_all(text, |caps: &regex::Captures| {
        let key = caps[1].trim();
        strings
            .get(key)
            .cloned()
            .unwrap_or_else(|| caps[1].to_string())
    });
    let ws = Regex::new(r"\s+").unwrap();
    ws.replace_all(&out, " ").trim().to_string()
}

fn engine_of_component(id: &str) -> Option<Engine> {
    match id {
        "CSZTBN012" => Some(Engine::Zapret),
        "CSZTBN062" => Some(Engine::Zapret2),
        "CSBIHA024" => Some(Engine::Byedpi),
        _ => None,
    }
}

fn flag_title(key: &str) -> String {
    match key {
        "useGameFilterTCP" => "Игровой фильтр TCP".into(),
        "useGameFilterUDP" => "Игровой фильтр UDP".into(),
        other => other.to_string(),
    }
}

fn tags_for(name: &str, desc: &str) -> Vec<String> {
    let hay = format!("{name} {desc}").to_lowercase();
    let mut out = Vec::new();
    for (tag, needles) in [
        ("youtube", &["youtube", "yt", "ютуб", "googlevideo"][..]),
        ("discord", &["discord", "дискорд"][..]),
        ("games", &["game", "игр", "valorant", "warp"][..]),
        ("meta", &["instagram", "facebook", "meta", "инста"][..]),
        ("tiktok", &["tiktok", "тикток"][..]),
        ("telegram", &["telegram", "телеграм"][..]),
    ] {
        if needles.iter().any(|n| hay.contains(n)) {
            out.push(tag.to_string());
        }
    }
    out
}

fn parse_kit(kit: &Path, kit_id: &str, kit_name: &str) -> Vec<Strategy> {
    let (strings, _lang) = load_strings(kit);
    let Ok(entries) = fs::read_dir(kit) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("json")
                && p.file_name().and_then(|n| n.to_str()) != Some("init.json")
                && p.file_name().and_then(|n| n.to_str()) != Some("manka-kit.json")
        })
        .collect();
    files.sort_by_key(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });

    let mut out = Vec::new();
    for file in files {
        let Some(obj) = read_json::<Value>(&file) else {
            continue;
        };
        let meta = str_field(&obj, "meta");
        if !meta.starts_with("IC:") && !meta.starts_with("UC:") {
            continue;
        }
        let target = obj
            .get("target")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let Some(engine) = engine_of_component(target) else {
            continue;
        };
        if !engine.supports_windows() {
            continue;
        }
        let Some(startup) = obj.get("startup_string").and_then(|v| v.as_str()) else {
            continue;
        };
        let stem = file
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| hash_id(startup));
        let display_name = localize(
            obj.get("name").and_then(|v| v.as_str()).unwrap_or(&stem),
            &strings,
        );

        let mut variables: BTreeMap<String, String> = BTreeMap::new();
        if let Some(vars) = obj.get("variables").and_then(|v| v.as_array()) {
            for v in vars {
                let Some(s) = v.as_str() else { continue };
                if let Some((k, val)) = s.split_once('=') {
                    let key = k.trim().trim_matches('%').trim().to_string();
                    if !key.is_empty() {
                        variables.insert(key, val.to_string());
                    }
                }
            }
        }
        if let Some(extra) = obj.get("commaVars").and_then(|v| v.as_object()) {
            for (k, v) in extra {
                if let Some(s) = v.as_str() {
                    variables.entry(k.clone()).or_insert_with(|| s.to_string());
                }
            }
        }

        let mut options = Vec::new();
        if let Some(arr) = obj
            .get("availableCommaVarsValues")
            .and_then(|v| v.as_array())
        {
            for entry in arr {
                let Some(variable) = entry.get("VarName").and_then(|v| v.as_str()) else {
                    continue;
                };
                let values: Vec<String> = entry
                    .get("Values")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                if values.is_empty() {
                    continue;
                }
                let title = localize(
                    entry
                        .get("Comment")
                        .and_then(|v| v.as_str())
                        .unwrap_or(variable),
                    &strings,
                );
                let default = variables
                    .get(variable)
                    .cloned()
                    .or_else(|| {
                        entry
                            .get("CurrentValueIndex")
                            .and_then(|v| v.as_u64())
                            .and_then(|idx| values.get(idx as usize).cloned())
                    })
                    .unwrap_or_default();
                options.push(StrategyOption {
                    variable: variable.to_string(),
                    title,
                    default_value: default,
                    values,
                });
            }
        }
        for o in &options {
            variables.remove(&o.variable);
        }

        let mut flags = Vec::new();
        if let Some(params) = obj.get("jparams").and_then(|v| v.as_object()) {
            for (k, v) in params {
                if let Some(b) = v.as_bool() {
                    flags.push(StrategyFlag {
                        key: k.clone(),
                        title: flag_title(k),
                        default: b,
                    });
                }
            }
        }

        let mut s = Strategy::new(
            format!("store-{kit_id}-{stem}"),
            engine,
            display_name,
            startup,
        );
        s.source = Source::Store;
        s.kit_id = Some(kit_id.to_string());
        s.description = Some(kit_name.to_string());
        s.variables = variables;
        s.options = options;
        s.game_filter = flags
            .iter()
            .any(|f| f.key.starts_with("useGameFilter") && f.default);
        s.flags = flags;
        s.tags = tags_for(&s.name, kit_name);
        // ports declared in the startup string, for display and default wf filters
        for token in args::split(startup) {
            if let Some(v) = token.strip_prefix("--wf-tcp=") {
                s.tcp_ports = Some(args::ports(v));
            }
            if let Some(v) = token.strip_prefix("--wf-udp=") {
                s.udp_ports = Some(args::ports(v));
            }
        }
        out.push(s);
    }
    out
}

pub fn kit_name(paths: &Paths, kit_id: &str) -> Option<String> {
    let v: Value = read_json(&paths.kit(kit_id).join("manka-kit.json"))?;
    v.get("name").and_then(|x| x.as_str()).map(str::to_string)
}

/// Presets of all installed kits.
pub fn installed_presets(paths: &Paths) -> Vec<Strategy> {
    let Ok(entries) = fs::read_dir(paths.kits()) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    let mut out = Vec::new();
    for dir in dirs {
        let kit_id = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let name = kit_name(paths, &kit_id).unwrap_or_else(|| kit_id.clone());
        out.extend(parse_kit(&dir, &kit_id, &name));
    }
    out
}

pub fn installed_kits_list(paths: &Paths) -> Vec<(String, String, String)> {
    installed_kits(paths)
        .into_iter()
        .map(|(id, version)| {
            let name = kit_name(paths, &id).unwrap_or_else(|| id.clone());
            (id, name, version)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localize_replaces_keys() {
        let mut map = BTreeMap::new();
        map.insert("x".to_string(), "Привет".to_string());
        assert_eq!(localize("$LOADSTRING(x) мир", &map), "Привет мир");
    }

    #[test]
    fn store_id_validation_blocks_traversal() {
        assert!(valid_store_id("CSZTR1003"));
        assert!(valid_store_id("ASGKOI001"));
        assert!(!valid_store_id(""));
        assert!(!valid_store_id(".."));
        assert!(!valid_store_id("../.."));
        assert!(!valid_store_id("a/b"));
        assert!(!valid_store_id("a\\b"));
        assert!(!valid_store_id("C:"));
        assert!(!valid_store_id("набор"));
        assert!(!valid_store_id("x".repeat(64).as_str()));
    }

    #[test]
    fn remove_kit_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let outside = dir.path().join("victim");
        std::fs::create_dir_all(&outside).unwrap();
        assert!(remove_kit(&paths, "..").is_err());
        assert!(remove_kit(&paths, "../victim").is_err());
        assert!(outside.exists());
        // a normal id works even when nothing is installed
        assert!(remove_kit(&paths, "CSZTR1003").is_ok());
    }
}
