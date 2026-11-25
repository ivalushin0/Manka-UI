# Manka for Windows

[![Release](https://github.com/ivalushin0/Manka-UI/actions/workflows/release.yml/badge.svg)](https://github.com/ivalushin0/Manka-UI/actions/workflows/release.yml)
[![CI](https://github.com/ivalushin0/Manka-UI/actions/workflows/ci.yml/badge.svg)](https://github.com/ivalushin0/Manka-UI/actions/workflows/ci.yml)
[![Downloads](https://img.shields.io/github/downloads/ivalushin0/Manka-UI/total?label=downloads)](https://github.com/ivalushin0/Manka-UI/releases)

[Download the latest release](https://github.com/ivalushin0/Manka-UI/releases/latest) · [Русская версия](README.md)

A DPI-circumvention hub for Windows — the desktop sibling of [Manka for Android](https://github.com/ivalushin0/manka).
One UI for **Zapret 2 / Zapret**, **TG WS Proxy** and **strategy auto-selection**. The app does not
implement any network trickery itself: it orchestrates the official engines (`winws2.exe`, `winws.exe`)
and the `tg-ws-proxy-rs` proxy.

> **Download only from GitHub.** There are no mirrors, file-sharing copies or "repacks".
> If Manka is offered anywhere else, it is not us.

## Features

- **DPI bypass**: runs `winws2`/`winws` as administrator, with status, uptime and logs.
- **Strategy watchdog**: background health checks of the active strategy; when your ISP tightens
  DPI ("it worked yesterday"), the watchdog re-runs auto-selection and applies the best strategy.
- **One-click diagnostics ("Doctor")**: privileges, conflicts with other bypass tools, the WinDivert
  driver, binary integrity, busy ports, autostart — plus a redacted diagnostics archive to attach to issues.
- **Components as plugins**: the official [bol-van/zapret-win-bundle](https://github.com/bol-van/zapret-win-bundle)
  and [tg-ws-proxy-rs](https://github.com/valnesfjord/tg-ws-proxy-rs) are installed from their original
  sources with **SHA256 verification** and **rollback** to the previous version.
- **Unified strategy catalogue**: built-in Windows strategies + kits from
  [CDPIUI-Store](https://github.com/Storik4pro/CDPIUI-Store) (IC:/UC: presets, variables, game filter).
- **Auto-selection**: TLS/HTTP/64-KB-body probes against YouTube, Discord, Telegram, Instagram,
  TikTok, Cloudflare and your own sites; ranked by success rate and latency.
- **Bypass scopes**: "list only" (banks, games and government sites go direct) or "all traffic".
- **Domain lists**: built-in list + scheduled updates of external lists
  ([Re-filter](https://github.com/1andrevich/Re-filter-lists)) with mirrors and custom sources;
  `manka-exclude.txt` keeps Russian services, banks and AI sites untouched.
- **Telegram**: local MTProto proxy on `127.0.0.1:1443`, `tg://` link, Cloudflare route check.
- **Tray**: close-to-tray, grey icon when the bypass is down, autostart via Task Scheduler (no UAC prompt).
- **Three UI languages**: русский, English, українська.

## Quick start (from source)

Requires: Windows 10/11 x64, Rust (MSVC), Node.js 20+, WebView2, Visual Studio Build Tools + Windows SDK.

```powershell
npm install
cd app; npm install; cd ..

# install components and activate a built-in strategy
cargo run -p manka-core --example setup

# run the app
.\node_modules\.bin\tauri.cmd dev
```

Run Manka **as administrator** (or press "Restart as administrator" in the yellow banner):
WinDivert cannot load otherwise.

Build the installer:

```powershell
.\node_modules\.bin\tauri.cmd build
# target\release\bundle\nsis\Manka_<version>_x64-setup.exe
```

## Security

The app runs as administrator (required by WinDivert), so it is built paranoically:

- TG WS Proxy and strategy kits are verified against the **SHA256 digests GitHub publishes**
  for every release asset; a mismatch aborts the install and deletes the file.
- The Zapret bundle is downloaded by an **immutable commit URL**, never a moving branch,
  and its hash is recorded at install time.
- The Doctor re-hashes installed executables to detect post-install tampering.
- Strict Content-Security-Policy in the WebView; all IPC commands validate their input.
- App updates are minisign-signed (tauri-plugin-updater).
- Settings are written atomically (`MoveFileEx` + `WRITE_THROUGH`); a corrupt settings file
  is backed up, never silently discarded.

**Antivirus flags?** WinDivert and `winws.exe` are historically flagged as "HackTool/Riskware"
by some vendors — GoodbyeDPI and zapret GUIs get the same. Manka downloads these files only from
the official repositories and verifies checksums; you can also build from source and compare.

## Data location

`%LOCALAPPDATA%\MankaData` (override with `MANKA_DATA_DIR`): settings, presets, components,
kits, lists, logs, autoselection history, store snapshot. Copy the whole folder to move to
another PC. See [docs/DATA.md](docs/DATA.md) for file formats.

## Limitations

- No SYSTEM service yet: engines are children of the app process (one UAC prompt at start
  unless launched via Task Scheduler autostart).
- Windows port does not manage DNS/hosts, per-app routing or Xray — planned after the service.
- Code is Windows-first; cross-platform builds are untested.

## License

Manka code — MIT. Components are distributed separately: zapret/zapret2 (MIT),
tg-ws-proxy-rs (MIT), CDPIUI-Store and kits — under their authors' licenses.
