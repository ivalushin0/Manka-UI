use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Where Manka keeps everything.
///
/// Default: `%LOCALAPPDATA%\Manka`. `MANKA_DATA_DIR` overrides it (portable
/// mode / tests). The Windows service will use `%ProgramData%\Manka`.
#[derive(Clone, Debug)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn detect() -> Self {
        if let Ok(dir) = std::env::var("MANKA_DATA_DIR") {
            if !dir.trim().is_empty() {
                return Self::new(dir);
            }
        }
        let base = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let root = base.join("MankaData");
        // Older builds stored data in `%LOCALAPPDATA%\Manka`, which is also
        // the NSIS per-user install directory. Move user data out of it so an
        // uninstall never deletes components or profiles.
        migrate_legacy_data(&base.join("Manka"), &root);
        Self::new(root)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ensure(&self) -> io::Result<()> {
        for dir in [
            self.root.clone(),
            self.components(),
            self.kits(),
            self.lists(),
            self.logs(),
            self.results(),
            self.store_index(),
            self.temp(),
        ] {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    pub fn presets_file(&self) -> PathBuf {
        self.root.join("presets.json")
    }

    pub fn components(&self) -> PathBuf {
        self.root.join("components")
    }

    pub fn component(&self, id: &str) -> PathBuf {
        self.components().join(id)
    }

    pub fn current_file(&self, id: &str) -> PathBuf {
        self.component(id).join("current.json")
    }

    pub fn kits(&self) -> PathBuf {
        self.root.join("kits")
    }

    pub fn kit(&self, id: &str) -> PathBuf {
        self.kits().join(id)
    }

    pub fn lists(&self) -> PathBuf {
        self.root.join("lists")
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn results(&self) -> PathBuf {
        self.root.join("results")
    }

    pub fn history_file(&self) -> PathBuf {
        self.results().join("history.json")
    }

    pub fn store_index(&self) -> PathBuf {
        self.root.join("store-index")
    }

    pub fn store_cache(&self) -> PathBuf {
        self.root.join("store-cache")
    }

    pub fn temp(&self) -> PathBuf {
        self.root.join("tmp")
    }

    pub fn exclude_list(&self) -> PathBuf {
        self.lists().join("manka-exclude.txt")
    }

    /// Hosts that are allowed to be desynced (the bypass whitelist/blacklist
    /// in zapret terms: the list of blocked resources).
    pub fn hostlist_file(&self) -> PathBuf {
        self.lists().join("manka-hostlist.txt")
    }

    /// Directory of the installed zapret bundle (works for both engines).
    pub fn engine_dir(&self) -> Option<PathBuf> {
        let info: ComponentCurrent =
            crate::util::read_json(&self.current_file("zapret"))?;
        let dir = self.component("zapret").join(info.dir);
        if dir.join("winws2.exe").exists() || dir.join("winws.exe").exists() {
            Some(dir)
        } else {
            None
        }
    }

    pub fn tgws_exe(&self) -> Option<PathBuf> {
        let info: ComponentCurrent = crate::util::read_json(&self.current_file("tgws"))?;
        let dir = self.component("tgws").join(info.dir);
        find_exe(&dir, "tg-ws-proxy")
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ComponentCurrent {    pub version: String,
    pub dir: String,
    #[serde(default)]
    pub installed_at: i64,
    #[serde(default)]
    pub previous: Option<String>,
}

fn find_exe(dir: &Path, stem: &str) -> Option<PathBuf> {
    let direct = dir.join(format!("{stem}.exe"));
    if direct.exists() {
        return Some(direct);
    }
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_exe(&path, stem) {
                return Some(found);
            }
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_lowercase().starts_with(&stem.to_lowercase()) && n.ends_with(".exe"))
            .unwrap_or(false)
        {
            return Some(path);
        }
    }
    None
}

pub fn write_current(paths: &Paths, id: &str, version: &str, dir: &str) -> io::Result<()> {
    let file = paths.current_file(id);
    let previous = crate::util::read_json::<ComponentCurrent>(&file)
        .map(|old| old.dir)
        .filter(|old| old != dir);
    crate::util::write_json(
        &file,
        &ComponentCurrent {
            version: version.to_string(),
            dir: dir.to_string(),
            installed_at: crate::util::now_ms(),
            previous,
        },
    )
}

pub fn read_current(paths: &Paths, id: &str) -> Option<ComponentCurrent> {
    crate::util::read_json(&paths.current_file(id))
}

/// One-time move of the pre-1.0 layout (`%LOCALAPPDATA%\Manka`) to
/// `%LOCALAPPDATA%\MankaData`.
fn migrate_legacy_data(legacy: &Path, root: &Path) {
    if root.exists() || !legacy.join("settings.json").exists() {
        return;
    }
    if fs::create_dir_all(root).is_err() {
        return;
    }
    for name in [
        "settings.json",
        "presets.json",
        "components",
        "kits",
        "lists",
        "logs",
        "results",
        "store-index",
        "tmp",
    ] {
        let src = legacy.join(name);
        if !src.exists() {
            continue;
        }
        let dst = root.join(name);
        if dst.exists() {
            continue;
        }
        let _ = fs::rename(&src, &dst);
    }
}
