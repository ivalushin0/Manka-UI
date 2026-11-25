use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Engine {
    #[default]
    Zapret2,
    Zapret,
    Byedpi,
}

impl Engine {
    pub fn id(self) -> &'static str {
        match self {
            Engine::Zapret2 => "zapret2",
            Engine::Zapret => "zapret",
            Engine::Byedpi => "byedpi",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Engine::Zapret2 => "Zapret 2",
            Engine::Zapret => "Zapret (legacy)",
            Engine::Byedpi => "ByeDPI",
        }
    }

    pub fn exe(self) -> &'static str {
        match self {
            Engine::Zapret2 => "winws2.exe",
            Engine::Zapret => "winws.exe",
            Engine::Byedpi => "ciadpi.exe",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "zapret2" => Some(Engine::Zapret2),
            "zapret" => Some(Engine::Zapret),
            "byedpi" => Some(Engine::Byedpi),
            _ => None,
        }
    }

    pub fn supports_windows(self) -> bool {
        matches!(self, Engine::Zapret2 | Engine::Zapret)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    #[default]
    Builtin,
    Store,
    User,
    Auto,
    Catalog,
    External,
    Zapretgui,
}

impl Source {
    pub fn title(self) -> &'static str {
        match self {
            Source::Builtin => "Встроенная",
            Source::Store => "Магазин",
            Source::User => "Своя",
            Source::Auto => "Автоподбор",
            Source::Catalog => "Каталог",
            Source::External => "Внешняя",
            Source::Zapretgui => "Zapret GUI",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct StrategyOption {
    pub variable: String,
    pub title: String,
    pub default_value: String,
    pub values: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct StrategyFlag {
    pub key: String,
    pub title: String,
    pub default: bool,
}

/// A strategy for one engine. `template` is a Windows command line: it may
/// contain `%NAME%` variables, `$GETCURRENTDIR()`, `$KIT`, `$ENGINE`,
/// `$LISTS`, `$EXCLUDE`, `{sni}` and CDPIUI `$LOCALCONDITION(...)`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Strategy {
    pub id: String,
    pub engine: Engine,
    pub name: String,
    #[serde(default)]
    pub source: Source,
    pub template: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub kit_id: Option<String>,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    #[serde(default)]
    pub options: Vec<StrategyOption>,
    #[serde(default)]
    pub flags: Vec<StrategyFlag>,
    #[serde(default)]
    pub tcp_ports: Option<String>,
    #[serde(default)]
    pub udp_ports: Option<String>,
    #[serde(default)]
    pub game_filter: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub score: Option<u32>,
    #[serde(default)]
    pub created_at: i64,
}

impl Strategy {
    pub fn new(
        id: impl Into<String>,
        engine: Engine,
        name: impl Into<String>,
        template: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            engine,
            name: name.into(),
            source: Source::Builtin,
            template: template.into(),
            description: None,
            kit_id: None,
            variables: BTreeMap::new(),
            options: Vec::new(),
            flags: Vec::new(),
            tcp_ports: None,
            udp_ports: None,
            game_filter: false,
            tags: Vec::new(),
            score: None,
            created_at: crate::util::now_ms(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct StoreItem {
    pub store_id: String,
    pub category: String,
    pub kind: String,
    pub name: String,
    pub short_name: String,
    pub developer: String,
    pub description: String,
    pub warning: Option<String>,
    pub version_control: String,
    pub link: String,
    pub preferred_file: Option<String>,
    pub installed_version: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ComponentStatus {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub installed: bool,
    pub version: Option<String>,
    pub latest: Option<String>,
    pub update_available: bool,
    pub path: Option<String>,
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProcessStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub uptime_sec: u64,
    pub engine: Option<String>,
    pub detail: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SiteResult {
    pub url: String,
    pub ok: u32,
    pub total: u32,
    pub avg_ms: u64,
    pub status: Option<u16>,
    pub bytes: u64,
    pub error: Option<String>,
}

impl SiteResult {
    pub fn percent(&self) -> u32 {
        self.ok
            .saturating_mul(100)
            .checked_div(self.total)
            .unwrap_or(0)
    }
}
