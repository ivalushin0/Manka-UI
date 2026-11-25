# Форматы данных Manka для Windows

Все файлы — UTF-8 JSON. Записи атомарные: временный файл рядом с целевым, затем
`MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)` — крах посреди
записи не оставляет ни обрезанный, ни пропавший файл.
Корень данных — `%LOCALAPPDATA%\MankaData` (переопределяется `MANKA_DATA_DIR`).
Повреждённый `settings.json` не затирается молча: копия сохраняется рядом как
`settings.corrupt-ГГГГММДД-ЧЧММСС.json`, событие пишется в `logs\app.log`.

## settings.json

```jsonc
{
  "enabled": true,                    // обход должен быть запущен
  "engine": "zapret2",                // zapret2 | zapret | byedpi
  "active": { "zapret2": "builtin-z2-faketls" },
  "option_value": { "store-...::YTDB_YTQC": 2 },
  "flag_value": { "store-...::useGameFilterTCP": false },
  "fake_sni": "www.google.com",
  "block_quic": true,
  "autostart": false,
  "start_minimized": true,            // запуск сразу в трей
  "close_to_tray": true,
  "debug_logs": false,

  "tgws_enabled": false,
  "tgws_port": 1443,
  "tgws_secret": "0123456789abcdef0123456789abcdef",
  "tgws_pool_size": 4,
  "tgws_buf_kb": 256,
  "tgws_cloudflare": true,
  "tgws_dc_ips": "2:149.154.167.220, 4:149.154.167.220",
  "tgws_extra": "",

  "targets": ["youtube", "discord", "general"],
  "custom_targets": "example.com",
  "autoselect_full": false,
  "autoselect_include_store": true,
  "autoselect_requests": 2,
  "autoselect_timeout": 8,
  "autoselect_stop_after_perfect": 2,
  "autoselect_targets": ["youtube", "discord"],

  "last_check": 0,
  "last_check_rate": -1,

  "update_interval_hours": 6,         // как часто проверять обновления компонентов
  "last_update_check": 0,
  "update_repo": "ivalushin0/Manka-UI", // репозиторий самообновления (owner/repo)
  "auto_update_check": true,
  "app_update_interval_hours": 24,
  "last_app_update_check": 0,

  "watchdog_enabled": true,           // сторож: перепроверка стратегии в фоне
  "watchdog_interval_min": 15,        // период проверки, минут
  "last_watchdog_check": 0,

  "lists_auto_update": false,         // автообновление внешних списков доменов
  "lists_update_interval_hours": 24,
  "lists_sources": [],                // свои URL (https) дополнительных списков
  "last_lists_update": 0,

  "language": "ru",
  "theme": "dark"
}
```

## presets.json

Массив пользовательских и автоматически выбранных стратегий:

```jsonc
[
  {
    "id": "auto-zapret2-1a2b3c4d5e6f7788",  // id включает движок: один шаблон может быть и для zapret, и для zapret2
    "engine": "zapret2",
    "name": "Zapret2 · FAKE TLS AUTO",
    "source": "auto",                 // builtin | store | user | auto | catalog
    "template": "--wf-tcp-out=80,443 --lua-desync=...",
    "description": "найдено автоподбором",
    "kit_id": null,
    "variables": {},
    "options": [
      { "variable": "YTDB_YTQC", "title": "QUIC", "default_value": "", "values": ["..."] }
    ],
    "flags": [
      { "key": "useGameFilterTCP", "title": "Игровой фильтр TCP", "default": false }
    ],
    "tcp_ports": "80,443",
    "udp_ports": "443",
    "game_filter": false,
    "tags": ["youtube", "discord"],
    "score": 100,
    "created_at": 1795269154469
  }
]
```

`template` может содержать:

- `%NAME%` — переменные набора (`variables`, `commaVars`);
- `$GETCURRENTDIR()`, `%~dp0` — папка набора;
- `$KIT`, `$ENGINE`, `$LUA`, `$FAKE`, `$LISTS`, `$EXCLUDE`;
- `{sni}` — `settings.fake_sni`;
- `$LOCALCONDITION(flag==true ? ... : ...)`;
- `$EMPTY` — пустая строка.

## components/<id>/current.json

```json
{
  "version": "6b4375fa04f6",
  "dir": "6b4375fa04f6",
  "installed_at": 1795269154469,
  "previous": "a1b2c3d4e5f6"
}
```

## kits/<store_id>/manka-kit.json

```json
{
  "name": "YtDisBystro",
  "version": "3.4",
  "store_id": "CSYOKS001",
  "source": "https://github.com/Storik4pro/YtDisBystro",
  "installed_at": 1795269154469
}
```

## results/history.json

Массив последних 30 прогонов:

```jsonc
[
  {
    "id": "1795269154469",
    "time": 1795269154469,
    "engine": "zapret2",
    "targets": ["https://www.youtube.com/", "https://discord.com/"],
    "baseline_ok": 0,
    "baseline_total": 4,
    "tested": 21,
    "results": [
      {
        "name": "Zapret2 · FAKE TLS AUTO",
        "template": "...",
        "strategy_id": "builtin-z2-faketls",
        "engine": "zapret2",
        "percent": 100,
        "ok": 4,
        "total": 4,
        "avg_ms": 706,
        "sites": [
          { "url": "https://www.youtube.com/", "ok": 2, "total": 2,
            "avg_ms": 640, "status": 200, "bytes": 65536, "error": null }
        ]
      }
    ]
  }
]
```

## logs

- `logs/app.log` — действия UI и автозапуск;
- `logs/engine.log` — командная строка winws, PID, код выхода, stderr/stdout;
- `logs/tgws.log` — то же для прокси.

Файлы обрезаются при превышении лимита (app — 4 МБ, engine перед прогоном — 2 МБ).
