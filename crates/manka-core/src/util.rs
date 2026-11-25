use std::fs;
use std::io;
use std::path::Path;

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Atomically write a file: write to a sibling temp file, then replace the
/// destination in one system call. On Windows this uses
/// `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`, so a
/// crash mid-write can never leave a half-written or missing settings file.
#[cfg(windows)]
pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    fs::write(&tmp, data)?;
    let from: Vec<u16> = tmp
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let to: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let ok = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        let err = io::Error::last_os_error();
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    Ok(())
}

/// Atomically write a file: write to a sibling temp file, then rename.
#[cfg(not(windows))]
pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)
}

/// SHA256 of a file as lowercase hex. Used to verify downloaded components
/// against the digests published by GitHub and to detect tampering later.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Verify a downloaded file against an expected SHA256 (hex, case-insensitive,
/// an optional `sha256:` prefix as returned by the GitHub API is accepted).
pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), String> {
    let expected = expected.trim().trim_start_matches("sha256:").to_lowercase();
    if expected.len() != 64 || !expected.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("Некорректный SHA256 в источнике: {expected}"));
    }
    let actual = sha256_file(path).map_err(|e| format!("Не удалось прочитать файл: {e}"))?;
    if actual != expected {
        let _ = fs::remove_file(path);
        return Err(format!(
            "Контрольная сумма не совпала: ожидалась {expected}, получена {actual}. Файл удалён."
        ));
    }
    Ok(())
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

pub fn append_log(path: &Path, line: &str) {
    use std::io::Write;
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

/// (pid, name, exe path) of running processes whose image name is in `names`.
/// Used by the Doctor to find conflicting DPI tools.
#[cfg(windows)]
pub fn find_processes(names: &[&str]) -> Vec<(u32, String, String)> {
    if names.is_empty() {
        return Vec::new();
    }
    let filter = names
        .iter()
        .map(|n| format!("Name='{n}'"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let script = format!(
        "Get-CimInstance Win32_Process -Filter \"{filter}\" | ForEach-Object {{ \"$($_.ProcessId)|$($_.Name)|$($_.ExecutablePath)\" }}"
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
        .output();
    let Ok(out) = out else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.trim().splitn(3, '|');
            let pid: u32 = parts.next()?.parse().ok()?;
            let name = parts.next()?.to_string();
            let path = parts.next().unwrap_or("").to_string();
            Some((pid, name, path))
        })
        .collect()
}

#[cfg(not(windows))]
pub fn find_processes(_names: &[&str]) -> Vec<(u32, String, String)> {
    Vec::new()
}

/// State of a Windows service (`sc query`), e.g. the WinDivert driver.
#[cfg(windows)]
pub fn service_state(name: &str) -> Option<String> {
    let out = std::process::Command::new("sc")
        .args(["query", name])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("STATE") {
            return rest.split_whitespace().nth(1).map(|s| s.to_string());
        }
    }
    None
}

#[cfg(not(windows))]
pub fn service_state(_name: &str) -> Option<String> {
    None
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
    let known =
        lower.contains("tgws") || lower.contains("tg-ws-proxy") || lower.contains("tgwsproxy");
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

pub fn sanitize_file_name(name: &str) -> String {
    name.chars()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_creates_and_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("nested").join("data.json");
        atomic_write(&file, b"one").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"one");
        // replacing an existing file must not fail and must not lose data
        atomic_write(&file, b"two").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"two");
        // no temp files left behind
        let leftovers: Vec<_> = fs::read_dir(file.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp files left: {leftovers:?}");
    }

    #[test]
    fn verify_sha256_accepts_and_rejects() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("blob.bin");
        fs::write(&file, b"manka").unwrap();
        // a digest that does not match must fail and delete the file
        let wrong = "e5b2d34dc22cc34d9d1b44862b046e6ea6a1af7a48b641f29a3e5769cb0f5f13";
        let actual = sha256_file(&file).unwrap();
        assert_eq!(actual.len(), 64);
        verify_sha256(&file, &actual).unwrap();
        verify_sha256(&file, &format!("sha256:{actual}")).unwrap();
        verify_sha256(&file, &actual.to_uppercase()).unwrap();
        assert!(verify_sha256(&file, wrong).is_err());
        // a failed verification removes the tampered file
        assert!(!file.exists());
        // malformed expectations are rejected without touching the file
        fs::write(&file, b"manka").unwrap();
        assert!(verify_sha256(&file, "zzz").is_err());
        assert!(file.exists());
    }

    #[test]
    fn sanitize_keeps_safe_chars() {
        assert_eq!(sanitize_file_name("a-b_c.d"), "a-b_c.d");
        assert_eq!(sanitize_file_name("../etc/passwd"), ".._etc_passwd");
        assert_eq!(sanitize_file_name("a/b\\c:d"), "a_b_c_d");
    }

    #[test]
    fn tail_lines_reads_last_n() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("log.txt");
        let text: String = (1..=50).map(|i| format!("line{i}\n")).collect();
        fs::write(&file, text).unwrap();
        let tail = tail_lines(&file, 3);
        assert_eq!(tail, vec!["line48", "line49", "line50"]);
        assert!(tail_lines(&dir.path().join("missing.log"), 5).is_empty());
    }
}
