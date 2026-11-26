use std::path::{Path, PathBuf};

use regex::Regex;

use crate::model::Engine;
use crate::paths::Paths;
use crate::settings::Settings;
use crate::Strategy;

/// Split a command line the way Windows tools expect: quotes group tokens,
/// backslash escapes the next character outside single quotes.
pub fn split(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut started = false;
    let chars = line.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    for c in chars {
        match c {
            '\'' if !in_double => {
                in_single = !in_single;
                started = true;
            }
            '"' if !in_single => {
                in_double = !in_double;
                started = true;
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if started {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            other => {
                cur.push(other);
                started = true;
            }
        }
    }
    if started {
        out.push(cur);
    }
    out
}

pub fn join(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if a.is_empty() || a.chars().any(|c| c.is_whitespace() || c == '"') {
                format!("\"{}\"", a.replace('"', "'"))
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Normalise a port list: "80, 443,50000-50100" -> "80,443,50000-50100".
pub fn ports(value: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for item in value.split([',', ' ', ';']) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let valid = item
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c == ':')
            && item
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false);
        if valid && !out.iter().any(|p| p == item) {
            let mut p = item.replace(':', "-");
            while p.contains("--") {
                p = p.replace("--", "-");
            }
            out.push(p);
        }
    }
    out.join(",")
}

#[derive(Clone, Debug)]
pub struct Rendered {
    pub args: Vec<String>,
    pub exe: Option<PathBuf>,
    pub working_dir: Option<PathBuf>,
    pub command_line: String,
}

fn norm_dir(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn condition_regex() -> Regex {
    Regex::new(
        r"\$LOCALCONDITION\(\s*([A-Za-z0-9_]+)\s*==\s*(true|false)\s*\?\s*([^:)]*?)\s*:\s*([^)]*?)\s*\)",
    )
    .expect("condition regex")
}

fn variable_regex() -> Regex {
    Regex::new(r"%([A-Za-z0-9_]+)%").expect("variable regex")
}

fn eval_conditions(text: &str, flags: &std::collections::BTreeMap<String, bool>) -> String {
    condition_regex()
        .replace_all(text, |caps: &regex::Captures| {
            let key = &caps[1];
            let expected = &caps[2] == "true";
            let actual = flags.get(key).copied().unwrap_or(false);
            if actual == expected {
                caps[3].to_string()
            } else {
                caps[4].to_string()
            }
        })
        .to_string()
}

fn has_arg(profile: &[String], prefix: &str) -> bool {
    profile.iter().any(|a| a.starts_with(prefix))
}

/// Restrict desync to the blocked-hosts list and, optionally, drop QUIC so
/// browsers fall back to TCP where the hostlist can be applied. When
/// `hostlist` is None (bypass scope "all") the desync applies to everything
/// and only the QUIC drop is left.
fn inject_hostlist(
    args: Vec<String>,
    hostlist: Option<&Path>,
    block_quic: bool,
    engine: Engine,
) -> Vec<String> {
    let usable = hostlist
        .map(|p| p.exists() && std::fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false))
        .unwrap_or(false);
    if !usable && !block_quic {
        return args;
    }
    let hostlist_arg = hostlist
        .map(|p| format!("--hostlist={}", norm_dir(p)))
        .unwrap_or_default();
    let mut out: Vec<String> = Vec::with_capacity(args.len() + 4);
    let mut profile: Vec<String> = Vec::new();

    let flush = |profile: &mut Vec<String>, out: &mut Vec<String>| {
        if profile.is_empty() {
            return;
        }
        let is_quic = profile.iter().any(|a| a.starts_with("--filter-l7=quic"));
        let is_http_tls = profile.iter().any(|a| {
            a.starts_with("--filter-l7=http")
                || a.starts_with("--filter-l7=tls")
                || a == "--payload=http_req"
                || a.starts_with("--payload=tls_client_hello")
        });
        let has_host_filter =
            has_arg(profile, "--hostlist=") || has_arg(profile, "--hostlist-domains=");
        // In "list" scope both TCP desync and the QUIC drop must be limited to
        // the hostlist. Dropping QUIC for the whole system makes normal sites
        // (Google sign-in, browsers using HTTP/3) hang until the TCP fallback.
        if usable && (is_http_tls || is_quic) && !has_host_filter {
            let pos = profile
                .iter()
                .position(|a| {
                    a.starts_with("--payload=")
                        || a.starts_with("--lua-desync=")
                        || a.starts_with("--dpi-desync=")
                })
                .unwrap_or(profile.len());
            profile.insert(pos, hostlist_arg.clone());
        }
        if block_quic && is_quic && (hostlist.is_none() || usable) {
            profile.retain(|a| !a.starts_with("--lua-desync=") && !a.starts_with("--dpi-desync="));
            profile.push(if engine == Engine::Zapret2 {
                "--lua-desync=drop".into()
            } else {
                "--dpi-desync=drop".into()
            });
        }
        out.append(profile);
    };

    for arg in args {
        if arg == "--new" {
            flush(&mut profile, &mut out);
            out.push(arg);
        } else {
            profile.push(arg);
        }
    }
    flush(&mut profile, &mut out);
    out
}

/// Substitute variables, options and conditions, then split into argv.
pub fn render(
    strategy: &Strategy,
    settings: &Settings,
    paths: &Paths,
    engine_dir: Option<&Path>,
    kit_dir: Option<&Path>,
) -> Result<Rendered, String> {
    // flags: saved value or the kit default
    let mut flags = std::collections::BTreeMap::new();
    for f in &strategy.flags {
        flags.insert(
            f.key.clone(),
            settings.flag(&strategy.id, &f.key, f.default),
        );
    }

    // values: %NAME%=... definitions; values themselves may contain conditions
    let mut values: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for (k, v) in &strategy.variables {
        values.insert(k.to_uppercase(), eval_conditions(v, &flags));
    }
    for opt in &strategy.options {
        let chosen = settings
            .option(&strategy.id, &opt.variable)
            .and_then(|idx| opt.values.get(idx).cloned())
            .unwrap_or_else(|| opt.default_value.clone());
        values.insert(opt.variable.to_uppercase(), chosen);
    }

    let mut line = eval_conditions(&strategy.template, &flags);
    let var_re = variable_regex();
    for _ in 0..5 {
        let next = var_re
            .replace_all(&line, |caps: &regex::Captures| {
                values
                    .get(&caps[1].to_uppercase())
                    .cloned()
                    .unwrap_or_else(|| caps[0].to_string())
            })
            .to_string();
        if next == line {
            break;
        }
        line = next;
    }

    let kit = kit_dir
        .map(norm_dir)
        .unwrap_or_else(|| norm_dir(&paths.kits()));
    let engine = engine_dir.map(norm_dir).unwrap_or_default();
    let engine_path = if engine.is_empty() {
        paths.engine_dir().map(|p| norm_dir(&p)).unwrap_or_default()
    } else {
        engine
    };

    line = line
        .replace("$GETCURRENTDIR()", &kit)
        .replace("%~dp0", &format!("{kit}/"))
        .replace("$KIT", &kit)
        .replace("$ENGINE", &engine_path)
        .replace("$LUA", &format!("{engine_path}/lua"))
        .replace("$FAKE", &format!("{engine_path}/files"))
        .replace("$LISTS", &norm_dir(&paths.lists()))
        .replace("$EXCLUDE", &norm_dir(&paths.exclude_list()))
        .replace("{sni}", &settings.fake_sni)
        .replace("$EMPTY", "");

    let mut args = split(&line);
    args.retain(|a| {
        !a.is_empty() && !a.starts_with("--ssid-filter") && !a.starts_with("--nlm-filter")
    });

    // winws2 needs its Lua library; add it when a preset does not bring its own.
    if strategy.engine == Engine::Zapret2
        && !args.iter().any(|a| a.starts_with("--lua-init="))
        && !engine_path.is_empty()
    {
        let lib = format!("{engine_path}/lua/zapret-lib.lua");
        let antidpi = format!("{engine_path}/lua/zapret-antidpi.lua");
        if Path::new(&lib).exists() {
            args.insert(0, format!("--lua-init=@{lib}"));
            if Path::new(&antidpi).exists() {
                args.insert(1, format!("--lua-init=@{antidpi}"));
            }
        }
    }

    // winws2 uses directional WinDivert filters; kit presets converted from
    // legacy often still carry --wf-tcp/--wf-udp, which winws2 rejects.
    if strategy.engine == Engine::Zapret2 {
        for a in &mut args {
            if let Some(rest) = a.strip_prefix("--wf-tcp=") {
                *a = format!("--wf-tcp-out={rest}");
            } else if let Some(rest) = a.strip_prefix("--wf-udp=") {
                *a = format!("--wf-udp-out={rest}");
            }
        }
    }

    // WinDivert filter is mandatory for winws: add a default one when the
    // preset only has --filter-tcp/--filter-udp blocks.
    let has_tcp_wf = args.iter().any(|a| {
        a.starts_with("--wf-tcp=")
            || a.starts_with("--wf-tcp-out=")
            || a.starts_with("--wf-tcp-in=")
    });
    let has_udp_wf = args.iter().any(|a| {
        a.starts_with("--wf-udp=")
            || a.starts_with("--wf-udp-out=")
            || a.starts_with("--wf-udp-in=")
    });
    let (tcp_key, udp_key) = if strategy.engine == Engine::Zapret2 {
        ("--wf-tcp-out", "--wf-udp-out")
    } else {
        ("--wf-tcp", "--wf-udp")
    };
    if !has_tcp_wf {
        let tcp = strategy
            .tcp_ports
            .as_deref()
            .map(ports)
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "80,443".to_string());
        args.insert(0, format!("{tcp_key}={tcp}"));
    }
    if !has_udp_wf {
        if let Some(udp) = strategy
            .udp_ports
            .as_deref()
            .map(ports)
            .filter(|p| !p.is_empty())
        {
            args.insert(0, format!("{udp_key}={udp}"));
        }
    }

    // drop empty profiles and stray --new; bypass scope "all" applies desync
    // to the whole TLS/HTTP traffic without a hostlist
    let hostlist = if settings.bypass_scope == "all" {
        None
    } else {
        Some(crate::lists::effective_hostlist(paths))
    };
    let args = inject_hostlist(
        args,
        hostlist.as_deref(),
        settings.block_quic,
        strategy.engine,
    );
    let mut cleaned: Vec<String> = Vec::with_capacity(args.len());
    for a in args {
        if a == "--new"
            && (cleaned.is_empty() || cleaned.last().map(|x| x == "--new").unwrap_or(false))
        {
            continue;
        }
        cleaned.push(a);
    }
    while cleaned.last().map(|a| a == "--new").unwrap_or(false) {
        cleaned.pop();
    }

    let exe = if strategy.engine.supports_windows() {
        paths
            .engine_dir()
            .map(|dir| dir.join(strategy.engine.exe()))
            .filter(|p| p.exists())
    } else {
        None
    };
    let working_dir = paths.engine_dir();

    Ok(Rendered {
        command_line: join(&cleaned),
        args: cleaned,
        exe,
        working_dir,
    })
}

/// Arguments for the Telegram proxy, built from settings.
pub fn tgws_args(settings: &Settings) -> Vec<String> {
    let mut args = vec![
        "--host".into(),
        "127.0.0.1".into(),
        "--port".into(),
        settings.tgws_port.to_string(),
        "--secret".into(),
        settings.tgws_secret.clone(),
        "--pool-size".into(),
        settings.tgws_pool_size.max(1).to_string(),
        "--buf-kb".into(),
        settings.tgws_buf_kb.max(64).to_string(),
    ];
    if settings.tgws_cloudflare {
        args.push("--default-domains".into());
    }
    let user_dcs: Vec<String> = settings
        .tgws_dc_ips
        .split([',', ' ', '\n', ';'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    // Without Cloudflare the proxy has nothing to connect to unless we pass
    // explicit DC targets. With --default-domains they would make CF slower,
    // so the built-in targets are only added for the direct-only mode.
    let dcs: Vec<String> = if user_dcs.is_empty() && !settings.tgws_cloudflare {
        vec!["2:149.154.167.220".into(), "4:149.154.167.220".into()]
    } else {
        user_dcs
    };
    for dc in dcs {
        args.push("--dc-ip".into());
        args.push(dc);
    }
    args.push("--ws-connect-timeout".into());
    args.push("3".into());
    args.push("--ip-fail-cooldown".into());
    args.push("60".into());
    if !settings.debug_logs {
        args.push("-q".into());
    }
    args.extend(split(&settings.tgws_extra));
    args
}

pub fn tgws_link(settings: &Settings) -> String {
    format!(
        "tg://proxy?server=127.0.0.1&port={}&secret=dd{}",
        settings.tgws_port, settings.tgws_secret
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Engine, StrategyFlag, StrategyOption};

    #[test]
    fn splits_quotes() {
        let args = split(r#"--filter-tcp=443 --hostlist="C:\my lists\yt.txt" --new"#);
        assert_eq!(args[0], "--filter-tcp=443");
        assert_eq!(args[1], r"--hostlist=C:\my lists\yt.txt");
        assert_eq!(args[2], "--new");
    }

    #[test]
    fn normalises_ports() {
        assert_eq!(ports("80, 443,50000-50100"), "80,443,50000-50100");
        assert_eq!(ports("a,80::90"), "80-90");
    }

    #[test]
    fn join_quotes_whitespace() {
        let joined = join(&["--a".to_string(), "b c".to_string()]);
        assert_eq!(joined, "--a \"b c\"");
    }

    /// A tempdir with a non-empty hostlist, so `inject_hostlist` is active.
    fn fixture() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        std::fs::create_dir_all(paths.lists()).unwrap();
        std::fs::write(paths.hostlist_file(), "youtube.com\n").unwrap();
        (dir, paths)
    }

    #[test]
    fn render_substitutes_variables_and_sni() {
        let (_dir, paths) = fixture();
        let mut s = Strategy::new(
            "t1",
            Engine::Zapret,
            "t",
            "--wf-tcp=%PORTS% --filter-tcp=443 --hostlist=$LISTS/x.txt --fake-sni={sni} --payload=%PAYLOAD%",
        );
        s.variables.insert("PORTS".into(), "443".into());
        s.options.push(StrategyOption {
            variable: "PAYLOAD".into(),
            title: "p".into(),
            default_value: "tls_client_hello".into(),
            values: vec!["tls_client_hello".into(), "http_req".into()],
        });
        let settings = Settings::default();
        let r = render(&s, &settings, &paths, None, None).unwrap();
        assert!(r.args.iter().any(|a| a == "--wf-tcp=443"));
        assert!(r.args.iter().any(|a| a == "--fake-sni=www.google.com"));
        assert!(r.args.iter().any(|a| a == "--payload=tls_client_hello"));
        // no unresolved placeholders left
        assert!(!r.args.iter().any(|a| a.contains('%') && a.contains("%")));
    }

    #[test]
    fn render_evaluates_conditions_with_flags() {
        let (_dir, paths) = fixture();
        let mut s = Strategy::new(
            "t2",
            Engine::Zapret,
            "t",
            "$LOCALCONDITION(game==true ? --game-on : --game-off) --wf-tcp=443",
        );
        s.flags.push(StrategyFlag {
            key: "game".into(),
            title: "g".into(),
            default: false,
        });
        let mut settings = Settings::default();
        let r = render(&s, &settings, &paths, None, None).unwrap();
        assert!(r.args.iter().any(|a| a == "--game-off"));
        settings.set_flag("t2", "game", true);
        let r = render(&s, &settings, &paths, None, None).unwrap();
        assert!(r.args.iter().any(|a| a == "--game-on"));
    }

    #[test]
    fn render_injects_hostlist_into_tls_profiles_only() {
        let (_dir, paths) = fixture();
        let s = Strategy::new(
            "t3",
            Engine::Zapret,
            "t",
            "--wf-tcp=80,443 --filter-tcp=443 --hostlist=own.txt --payload=tls_client_hello --dpi-desync=fake --new \
             --filter-tcp=443 --payload=tls_client_hello --dpi-desync=fake --new --filter-udp=443 --dpi-desync=fake",
        );
        let settings = Settings {
            block_quic: false,
            ..Default::default()
        };
        let r = render(&s, &settings, &paths, None, None).unwrap();
        let hostlist = format!(
            "--hostlist={}",
            paths.hostlist_file().to_string_lossy().replace('\\', "/")
        );
        // profile 1 already has its own hostlist: untouched
        assert!(r.args.iter().any(|a| a == "--hostlist=own.txt"));
        // profile 2 (TLS, no hostlist) got the injected one
        assert!(r.args.contains(&hostlist));
        // profile 3 is UDP: no injection there
        let udp_pos = r.args.iter().position(|a| a == "--filter-udp=443").unwrap();
        assert!(!r.args[udp_pos..].contains(&hostlist));
    }

    #[test]
    fn render_drops_quic_when_requested() {
        let (_dir, paths) = fixture();
        let s = Strategy::new(
            "t4",
            Engine::Zapret2,
            "t",
            "--wf-udp=443 --filter-l7=quic --lua-desync=fake",
        );
        let settings = Settings::default(); // block_quic = true
        let r = render(&s, &settings, &paths, None, None).unwrap();
        assert!(r.args.iter().any(|a| a == "--lua-desync=drop"));
        assert!(!r.args.iter().any(|a| a == "--lua-desync=fake"));
        // scope=list: the QUIC drop must be limited to the hostlist too
        let hostlist = format!(
            "--hostlist={}",
            paths.hostlist_file().to_string_lossy().replace('\\', "/")
        );
        assert!(
            r.args.contains(&hostlist),
            "QUIC drop must be hostlist-scoped: {:?}",
            r.args
        );
    }

    #[test]
    fn render_scope_all_drops_quic_everywhere() {
        let (_dir, paths) = fixture();
        let s = Strategy::new(
            "t4b",
            Engine::Zapret2,
            "t",
            "--wf-udp=443 --filter-l7=quic --lua-desync=fake",
        );
        let settings = Settings {
            bypass_scope: "all".into(),
            ..Default::default()
        };
        let r = render(&s, &settings, &paths, None, None).unwrap();
        assert!(r.args.iter().any(|a| a == "--lua-desync=drop"));
        assert!(!r.args.iter().any(|a| a.starts_with("--hostlist=")));
    }

    #[test]
    fn render_converts_wf_filters_for_zapret2() {
        let (_dir, paths) = fixture();
        let s = Strategy::new(
            "t5",
            Engine::Zapret2,
            "t",
            "--wf-tcp=80,443 --wf-udp=443 --filter-tcp=443 --lua-desync=fake",
        );
        let settings = Settings::default();
        let r = render(&s, &settings, &paths, None, None).unwrap();
        assert!(r.args.iter().any(|a| a == "--wf-tcp-out=80,443"));
        assert!(r.args.iter().any(|a| a == "--wf-udp-out=443"));
        assert!(!r.args.iter().any(|a| a.starts_with("--wf-tcp=")));
        assert!(!r.args.iter().any(|a| a.starts_with("--wf-udp=")));
    }

    #[test]
    fn render_adds_default_wf_when_missing() {
        let (_dir, paths) = fixture();
        let s = Strategy::new(
            "t6",
            Engine::Zapret,
            "t",
            "--filter-tcp=443 --dpi-desync=fake",
        );
        let settings = Settings::default();
        let r = render(&s, &settings, &paths, None, None).unwrap();
        assert_eq!(r.args.first().map(|s| s.as_str()), Some("--wf-tcp=80,443"));
    }

    #[test]
    fn render_never_leaves_dangling_new() {
        let (_dir, paths) = fixture();
        let s = Strategy::new("t7", Engine::Zapret, "t", "--new --new --wf-tcp=443 --new");
        let settings = Settings::default();
        let r = render(&s, &settings, &paths, None, None).unwrap();
        assert_ne!(r.args.last().map(|s| s.as_str()), Some("--new"));
        assert!(!r
            .args
            .windows(2)
            .any(|w| w[0] == "--new" && w[1] == "--new"));
    }

    #[test]
    fn render_strips_ssid_filters() {
        let (_dir, paths) = fixture();
        let s = Strategy::new(
            "t8",
            Engine::Zapret,
            "t",
            "--wf-tcp=443 --ssid-filter=home --nlm-filter=any --dpi-desync=fake",
        );
        let settings = Settings::default();
        let r = render(&s, &settings, &paths, None, None).unwrap();
        assert!(!r.args.iter().any(|a| a.starts_with("--ssid-filter")));
        assert!(!r.args.iter().any(|a| a.starts_with("--nlm-filter")));
    }

    #[test]
    fn render_scope_all_skips_hostlist() {
        let (_dir, paths) = fixture();
        let s = Strategy::new(
            "t9",
            Engine::Zapret,
            "t",
            "--wf-tcp=443 --filter-tcp=443 --payload=tls_client_hello --dpi-desync=fake",
        );
        let settings = Settings {
            bypass_scope: "all".into(),
            block_quic: false,
            ..Default::default()
        };
        let r = render(&s, &settings, &paths, None, None).unwrap();
        assert!(
            !r.args.iter().any(|a| a.starts_with("--hostlist=")),
            "scope=all must not inject a hostlist: {:?}",
            r.args
        );
    }

    #[test]
    fn tgws_args_are_stable() {
        let settings = Settings::default();
        let args = tgws_args(&settings);
        assert!(args
            .windows(2)
            .any(|w| w[0] == "--host" && w[1] == "127.0.0.1"));
        assert!(args
            .windows(2)
            .any(|w| w[0] == "--secret" && w[1] == settings.tgws_secret));
        assert!(args.iter().any(|a| a == "--default-domains"));
        assert!(args.iter().any(|a| a == "-q"));
        let link = tgws_link(&settings);
        assert!(link.starts_with("tg://proxy?server=127.0.0.1&port=1443&secret=dd"));
    }
}
