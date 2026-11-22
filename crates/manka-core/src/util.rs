use std::fs;
use std::io;
use std::path::Path;

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Atomically write a file: write to a sibling temp file, then rename.
pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    fs::write(&tmp, data)?;
    // Windows rename fails when the destination exists.
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&tmp, path)
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let text = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    atomic_write(path, &text)
}

/// Keep only the last `n` newline-separated lines of a file.
pub fn tail_lines(path: &Path, n: usize) -> Vec<String> {
    use std::io::{Read, Seek, SeekFrom};

    let n = n.max(1);
    let Ok(mut file) = fs::File::open(path) else {
        return Vec::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    // Engine logs contain very long command lines; reading the tail window is
    // enough for the UI and keeps the IPC payload small even on 4 MB logs.
    const MAX_BYTES: u64 = 512 * 1024;
    let start = len.saturating_sub(MAX_BYTES);
    if start > 0 && file.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut text: &str = &text;
    if start > 0 {
        if let Some(pos) = text.find('\n') {
            text = &text[pos + 1..];
        }
    }
    let lines: Vec<&str> = text.lines().collect();
    let from = lines.len().saturating_sub(n);
    lines[from..].iter().map(|s| s.to_string()).collect()
}

/// Truncate a log file if it grew beyond `limit` bytes.
pub fn trim_log(path: &Path, limit: u64) {
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() > limit {
            let _ = fs::write(path, b"");
        }
    }
}

pub fn append_log(path: &Path, line: &str) {    use std::io::Write;
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(f, "{ts} {line}");
    }
    trim_log(path, 4 * 1024 * 1024);
}

/// Kill leftover engine/proxy processes started by a previous Manka run
/// (e.g. after a crash) so the new instance owns them. Processes outside the
/// Manka components directory (CDPI, zapretgui) are never touched.
#[cfg(windows)]
pub fn kill_stale_components(components: &Path) -> usize {
    let root = components.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$root='{root}'; Get-CimInstance Win32_Process -Filter \"Name='winws.exe' OR Name='winws2.exe' OR Name='tg-ws-proxy.exe'\" | Where-Object {{ $_.ExecutablePath -and $_.ExecutablePath.StartsWith($root, 'OrdinalIgnoreCase') }} | ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue; $_.ProcessId }}"
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| l.trim().parse::<u32>().is_ok())
            .count(),
        Err(_) => 0,
    }
}

#[cfg(not(windows))]
pub fn kill_stale_components(_components: &Path) -> usize {
    0
}

/// (pid, process name) of the process listening on `port`, if any.
#[cfg(windows)]
pub fn port_owner(port: u16) -> Option<(u32, String)> {
    let out = std::process::Command::new("netstat")
        .args(["-ano", "-p", "tcp"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let needle = format!(":{port}");
    for line in text.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 5
            && cols[0].eq_ignore_ascii_case("TCP")
            && cols[1].ends_with(&needle)
            && cols[3].eq_ignore_ascii_case("LISTENING")
        {
            let pid: u32 = cols[4].parse().ok()?;
            let name = process_name(pid).unwrap_or_else(|| format!("PID {pid}"));
            return Some((pid, name));
        }
    }
    None
}

#[cfg(not(windows))]
pub fn port_owner(_port: u16) -> Option<(u32, String)> {
    None
}

#[cfg(windows)]
fn process_name(pid: u32) -> Option<String> {
    let out = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next()?.trim();
    if first.starts_with("INFO:") || first.contains("No tasks") {
        return None;
    }
    // "name.exe","pid",...
    first
        .split(',')
        .next()
        .map(|s| s.trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
}

/// Kill the listening process if it is a known Telegram-proxy build.
#[cfg(windows)]
pub fn stop_port_owner(port: u16) -> Result<String, String> {
    let (pid, name) = port_owner(port).ok_or_else(|| format!("Порт {port} свободен"))?;
    let lower = name.to_lowercase();
    let known = lower.contains("tgws") || lower.contains("tg-ws-proxy") || lower.contains("tgwsproxy");
    if !known {
        return Err(format!(
            "Порт {port} занят программой {name} (PID {pid}). Закройте её вручную или смените порт."
        ));
    }
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F", "/T"])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("Не удалось остановить {name} (PID {pid})"));
    }
    std::thread::sleep(std::time::Duration::from_millis(400));
    Ok(name)
}

#[cfg(not(windows))]
pub fn stop_port_owner(_port: u16) -> Result<String, String> {
    Err("только Windows".into())
}

/// Is anything listening on the local port?
pub fn port_busy(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        std::time::Duration::from_millis(250),
    )
    .is_ok()
}

pub fn sanitize_file_name(name: &str) -> String {    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn hash_id(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    hex::encode(&h.finalize()[..8])
}

/// Running elevated? Used by the UI to explain why the engine cannot start.
#[cfg(windows)]
pub fn is_elevated() -> bool {
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len: u32 = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    true
}

/// Relaunch the current executable with a UAC prompt.
#[cfg(windows)]
pub fn relaunch_elevated() -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_w = wide(exe.as_os_str());
    let verb = wide(OsStr::new("runas"));
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            exe_w.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if result > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecuteW failed: {result}"))
    }
}

#[cfg(not(windows))]
pub fn relaunch_elevated() -> Result<(), String> {
    Err("elevation is only needed on Windows".into())
}
