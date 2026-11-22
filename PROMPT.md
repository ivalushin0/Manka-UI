# ПРОМТ: создать кроссплатформенный «хаб» для Zapret2 + TG WS Proxy с автоподбором стратегий

> Скопируй этот файл целиком в новый чат на другом компьютере как задание. Он самодостаточен:
> ниже описано, что уже существует, что нужно сделать, какие данные использовать и как проверять результат.

---

## 0. Роль

Ты — старший инженер-разработчик Windows-приложений. Нужно спроектировать и реализовать
**единый хаб** (один установщик, без лишних иконок в трее) для управления обходом DPI:
Zapret2 (winws2.exe), legacy Zapret (winws.exe), TG WS Proxy, списки доменов/IP и пресеты стратегий,
с **автоматическим подбором стратегии** под конкретную сеть провайдера.

## 1. Что уже существует (анализ рынка, изучить и переиспользовать идеи)

### 1.1. CDPI UI (github.com/Storik4pro/cdpiui, Apache-2.0, C# .NET 8 WinUI3)

Функции:
- Магазин компонентов (`Storik4pro/CDPIUI-Store`) — JSON-манифесты `Components/<name>/init.json`:
  поля `version_control` (`git` = релиз-тег, `git_only_last` = последний коммит, `local`),
  `version_control_link`, `download_link`, `filetype` (`archive`/`WIN32application`),
  `archive_root_folder`, `target_executable_file`, `target_minversion/maxversion`.
- Компоненты: Zapret (winws v72), Zapret2 (winws2), GoodbyeDPI, ByeDPI, SpoofDPI, NoDPI, TG WS Proxy.
- Паки конфигов (legacy zapret v1 JSON): YtDisBystro, zapret-discord-youtube, zapret-discord-youtube-alt,
  CSZEFL002, CSYOKS001. Формат: `{meta:"IC:v1.0", name, target:[binary,version], jparams:{useGameFilter},
  variables:["%BIN%=...","%LISTS%=..."], startup_string:"--wf-tcp=... --filter-... --dpi-desync=..."}`.
- Конвертация legacy-конфигов в zapret2 (Lua-совместимый запуск), кэш `__ConvertedZPRT` + манифесты с `InputHash`.
- Редактор хост-листов и файла hosts, автоподбор параметров запуска (BlockCheck2), тест-окно конфига,
  логи, "условные задачи", автозапуск, трей, сервис-менеджер, обмен пресетами (файлом).
- Локальная SQLite-база `Store/Cache/Local/storedata.db`, таблица `Items`
  (`Id, Type, Directory, Executable, UpdateCheckUrl, DownloadUrl, DownloadFileType, VersionControlType,
  CurrentVersion, ...`). Если `CurrentVersion` пуст — компонент переустанавливается при каждом старте.
- Память в фоне ~8 МБ, диск ~135 МБ с компонентами.

### 1.2. Zapret 2 GUI / «Zapret Dev» (github.com/zapret-discordyoutube/zapretgui)

- Встроенная библиотека пресетов: `src/presets/builtin/winws1/*.txt` (~20 файлов: YTDisBystro 3.4,
  Shigulovski, Ufanet, alt1..alt10, Default v1) и `src/presets/builtin/winws2/*.txt` (~130 файлов:
  general ALT1–ALT13, FAKE TLS AUTO, SIMPLE FAKE, hostfakesplit, multidisorder, multisplit, game-filter
  варианты, «Ростелеком», circular-стратегии и т.д.). Пресет = текстовый файл с аргументами winws.
- Управление пресетами: папки, импорт по URL, рейтинги, удалённая синхронизация, raw-редактор,
  контракт пресета/миграции, диагностика покрытия, переносимый архив (export/import).
- **Диагностика и автоподбор**: `src/blockcheck/*`:
  `strategy_search/engine.py` (поиск), `probes.py`, `verdict.py`, `ordering.py`, `sni_discovery.py`,
  `environment.py`, `history.py`, `winws_session.py`; `strategy_scan_*` (применение, воркеры, планы, состояние);
  также `browser_check.py`, `browser_netlog.py`, `googlevideo_discovery.py`, `stun_tester.py`,
  `targets.py`, `server_matrix.py`, UI-результаты и карточки.
- Дополнительно: DNS local proxy (`src/dns/local_proxy*`), управление hosts/proxy_domains,
  фасад Telegram-прокси (`src/app/feature_facades/telegram_proxy.py`) и проверка (`diagnostics/telegram_check.py`),
  страница Windows-функций, контроль uptime/статуса.

### 1.3. TG WS Proxy (Flowseal/tg-ws-proxy, Python)
- Локальный MTProto-прокси: `--host 127.0.0.1 --port 1443 --secret <hex> --buf-kb 256 --pool-size 4 --dc-ip ...`.
- Версии 1.6.5 — консольная (без трея), 1.6.6+ — GUI/tray (в коде есть ветка console mode, если нет pystray).
- В версии 1.6.5 утечка сокетов (сотни CLOSE_WAIT → ломаются новые TCP-соединения во всей системе).

## 2. Проблема, которую решаем

1. Компоненты обновляются вручную/с задержкой; при ручной подмене файлов CDPI переустанавливает их каждый запуск.
2. Каждый компонент — отдельное приложение со своим треем; хочется один хаб.
3. Стратегии разбросаны (CDPI-паки, zapretgui-пресеты, апстрим zapret2) — нет единого каталога и автоподбора.
4. Хочется один маленький установщик, мало памяти, автообновления и работа на других ПК.

## 3. Требования

### 3.1. Функциональные
1. **Компоненты как плагины**: winws.exe, winws2.exe, tg-ws-proxy, инструменты диагностики; манифесты,
   установка/обновление/откат, включение/выключение, автозапуск.
2. **Никаких отдельных иконок в трее**: компоненты — дочерние процессы службы; TG-прокси — headless-сборка.
3. **Один установщик** + portable-режим (без службы, UAC при запуске).
4. **Автообновления** компонентов из первоисточников:
   - Zapret2 ← `bol-van/zapret-win-bundle` (master.zip, `zapret-winws/`);
   - Zapret(legacy) ← тот же бандл (`winws.exe`);
   - TG WS Proxy ← последний релиз `Flowseal/tg-ws-proxy` (`TgWsProxy_windows.exe`);
   - списки/пресеты ← CDPIUI-Store / zapretgui / апстрим.
5. **Единый каталог стратегий/пресетов** с импортом из CDPI-паков (legacy v1) и zapretgui (`builtin/winws1|winws2/*.txt`).
6. **Автоподбор стратегии** (см. §6) с сохранением результата по «отпечатку сети».
7. **Управление списками**: hostlist/ipset/exclude, дедупликация, лимиты, привязка к профилю.
8. **Профили**: набор (компонент + стратегия + списки + фильтры + game filter + автозапуск).
9. **Telegram**: встроенный компонент прокси, статус, кнопка «подключить» (ссылка tg://), проверка.
10. **UX обновлений**: «Пуск» сразу поднимает текущую версию, обновление — в фоне, применяется при следующем старте.
11. **Диагностика**: проверка блокировок (как blockcheck), отчёт по целям (youtube/discord/telegram/произвольные),
    скорость/латентность/ошибки, экспорт отчёта.
12. **Логи/история**: запуски, обновления, результаты тестов.

### 3.2. Нефункциональные
- Windows 10 1809+ / 11, x64 (по возможности ARM64).
- Установщик ~10–15 МБ, установка с компонентами ≤ 40 МБ (+списки по желанию).
- Фон (служба): ≤ 15 МБ RAM; UI: ≤ 120 МБ только когда открыт.
- Ноль UAC-окон после установки (служба запускает winws2 от SYSTEM).
- Ноль постоянных треев; опционально один трей у хаба (настройка).
- Все сетевые загрузки: HTTPS + SHA256 + (опционально) Ed25519-подпись манифеста.
- Полная офлайн-работа после установки (кроме проверки обновлений).

## 4. Технологический стек (решение)

**Rust + Tauri v2** (бэкенд Rust, UI Svelte/TypeScript), Windows-first, кроссплатформенность в перспективе.
Обоснование: размер (10–15 МБ), память, скорость старта, WebView2 уже есть в Win10/11, единый бинарник службы.

Альтернативы (если Rust неприемлем): Go + Wails; C# + Avalonia (больше вес/зависимость .NET Runtime).
Запрещено: Electron (тяжёлый), Python-UI в проде (медленный старт).

## 5. Архитектура

```
hub-ui.exe  (окно по кнопке; опц. один трей)  ── IPC (named pipe, токен) ──►  hubd (служба, SYSTEM)
                                                                             ├─ winws2.exe / winws.exe
                                                                             ├─ tgwsproxy (headless)
                                                                             └─ updater / diagnostics
```

- Служба: управление процессами, обновления, каталоги, тесты, откаты, watchdog, автозапуск.
- UI: без бизнес-логики; всё через IPC; состояния и прогресс — события.
- Компоненты: `%ProgramData%\<App>\components\<id>\<version>` + `current` (junction/указатель) + `previous` (откат).
- Данные пользователя: `%ProgramData%\<App>\profiles`, `lists`, `results`, `logs`.
- Секреты (secret TG-прокси) — не в репозитории; генерируются локально.

## 6. Автоподбор стратегии (ключевая функция)

Алгоритм:
1. Собрать цели: youtube (www.youtube.com, i.ytimg.com, googlevideo), discord (discord.com, cdn.discordapp.com, gateway),
   telegram (DC + api), плюс пользовательские домены. Для каждой — метод проверки:
   TLS-хендшейк, HTTP-код, TTFB, скорость первых N КБ, для видео — загрузка сегмента.
2. Отпечаток сети: ASN/ISP (по внешнему IP), MAC-хэш шлюза, DNS-серверы. Кэш: `network_fingerprint → лучшая стратегия`.
3. Кандидаты: отфильтрованный каталог (по тегам: youtube/discord/games), приоритет — ранее успешные.
4. Для каждого кандидата: запустить winws с пресетом, прогнать цели, зафиксировать метрики, остановить.
   Обязательны таймауты и защита от «залипания» (убийство процесса по таймауту).
5. Вердикт: pass/fail по каждой цели + агрегированный балл (успех × скорость − штраф за ошибки).
6. Ранжирование, выбор лучшего, применение, запись в историю. При падении метрик в будущем — авто-перепроверка
   (например, раз в сутки или по кнопке) и предложение замены с откатом.
7. «Быстрый» режим (5–10 стратегий, 3 цели) и «полный» (весь каталог, все цели).
8. Не тестировать на реальном трафике игры; тесты — только на выделенных целях.

## 7. Модели данных (обязательные схемы)

```jsonc
// component.json
{ "id": "zapret2", "name": "Zapret2", "kind": "engine", "arch": ["x64","arm64"],
  "provider": { "type": "zip_bundle|github_release_asset|git_tag|local",
                "url": "…", "root": "…", "asset_pattern": "TgWsProxy_windows.exe" },
  "versioning": { "type": "fingerprint", "fingerprint_from": "winws2.exe" },
  "install": { "target": "winws2.exe", "preserve": ["lua/*"] },
  "run": { "exe": "winws2.exe", "args": "{strategy_args}", "working_dir": ".", "elevation": "system" },
  "health": { "process": "winws2.exe", "probe": null },
  "update": { "interval_hours": 6 } }

// strategy.json (нормализованный пресет)
{ "id": "zapretgui/winws2/general ALT9",
  "name": "general ALT9", "engine": "winws2", "source": "zapretgui",
  "args": ["--wf-tcp=80,443", "--filter-tcp=80,443", "--hostlist=list-general.txt", "..."],
  "placeholders": { "list-general.txt": {"type":"ref","id":"list-general"} },
  "tags": ["youtube","discord"], "game_filter": false,
  "min_engine_version": "1.0.7", "hash": "sha256:…" }

// profile.json
{ "id":"home", "name":"Дом", "component":"zapret2", "strategy":"zapretgui/winws2/general ALT9",
  "lists": {"list-general":"russia-blacklist"}, "game_filter": false,
  "telegram_proxy": {"enabled":true,"port":1443}, "autostart": true }

// result.json
{ "profile":"home", "fingerprint":"AS12345:gw=aa:bb", "ts":"…",
  "targets":[{"id":"youtube","ok":true,"tls_ms":112,"ttfb_ms":240,"kbps":12000}],
  "score": 0.93, "verdict":"pass" }
```

## 8. Импорт стратегий и списков

1. **zapretgui**: `src/presets/builtin/winws1/*.txt`, `winws2/*.txt` — текст с аргументами winws.
   Парсер: строки → аргументы; подстановки `%BIN%`, `%LISTS%` → наши пути; имя файла → `name`.
2. **CDPI-паки**: `*.json` с `startup_string` и `variables`; для `target` с winws — движок `winws1`,
   для winws2-конвертированных — `winws2`; `jparams.useGameFilter` → `game_filter`.
3. **Апстрим zapret2**: нативные конфиги/списки (`bol-van/zapret2`).
4. **Списки**: `russia-blacklist.txt` (1,8 МБ), `russia-youtube*.txt`, `cloudflare-ipset*.txt`,
   `list-general.txt`, `list-exclude.txt`, `ipset-all.txt` — с дедупликацией и предупреждением о «широких» списках.

## 9. Экраны UI (минимум)

1. **Главная**: статус (движок, прокси, профиль), кнопка Пуск/Стоп, uptime, «прогнать диагностику».
2. **Профили**: создание/выбор, привязка компонента, стратегии, списков, game filter, автозапуск.
3. **Стратегии**: каталог с фильтрами по тегам/источнику, поиск, избранное, «применить», «проверить».
4. **Диагностика**: выбор целей, быстрый/полный прогон, результаты по целям, серверная матрица, экспорт.
5. **Компоненты**: версии, обновить, откатить, лог обновлений, автопоиск обновлений.
6. **Telegram**: статус прокси, копирование ссылки, «подключить», проверка.
7. **Списки**: просмотр/редактирование, импорт, дедупликация, размеры.
8. **Настройки**: язык, тема, порт прокси, интервал обновлений, portable/служба, трей вкл/выкл, логи.

## 10. Этапы

| Этап | Результат | Критерий готовности |
|---|---|---|
| M0 | Каркас: служба + UI + IPC, запуск/останов winws2 с текущим конфигом | Пуск/Стоп работает без UAC, процесс живёт после закрытия UI |
| M1 | Менеджер компонентов (манифесты, провайдеры, версии, откат) | Обновление Zapret2 и TG-прокси «одной кнопкой», повторно не переустанавливает |
| M2 | Headless TG WS Proxy как компонент | Прокси работает, трея нет, Telegram подключается, утечки сокетов нет |
| M3 | Каталог стратегий + импорт CDPI-паков | ≥ 20 стратегий импортировано, применяются |
| M4 | Импорт пресетов zapretgui + тестер + автоподбор | Быстрый прогон выбирает рабочую стратегию, результат кэшируется |
| M5 | Установщик, portable, подписи, самообновление | Один `.exe`-установщик, установка на чистую Win10/11, размер в бюджете |

## 11. Критерии приёмки

- Установка на чистую Windows 10/11 x64 без .NET/Python; запуск от обычного пользователя.
- После установки в трее **ноль** иконок компонентов; опционально один трей хаба.
- `winws2` стартует без UAC; TG-прокси работает на 127.0.0.1:1443.
- Обновление Zapret2 (bundle) и TG-прокси (release) — автоматически, с откатом при ошибке.
- Повторный запуск не переустанавливает компоненты (fingerprint совпал).
- Импорт минимум 20 стратегий из CDPI и 20 из zapretgui; применение любой — успешно.
- Автоподбор: на тестовом стенде выбирает стратегию с pass по youtube+discord, время полного прогона < 10 мин.
- Размер установщика ≤ 20 МБ; служба ≤ 15 МБ RAM; UI ≤ 120 МБ.
- Логи и понятные ошибки при сбое компонента; откат в 1 действие.

## 12. Не входит в проект

- Своя реализация DPI-обхода (пишем только оркестратор; движки — внешние бинарники).
- VPN-клиенты, прокси для других сервисов.
- Кроссплатформенный UI сверх Win/Linux (macOS — по возможности).

## 13. Структура репозитория

```
/app            (Tauri: UI Svelte/TS)
/src-tauri      (Rust: служба, IPC, updater, runner)
/crates
  /core         (модели: component, strategy, profile, result)
  /providers    (github_release_asset, zip_bundle, git_tag, local)
  /runner       (процессы, watchdog, elevation, логи)
  /importer     (CDPI, zapretgui, zapret2)
  /tester       (probes, verdict, scoring)
/data           (builtin strategies index, schemas)
/docs           (ARCHITECTURE, PROMPT, контракты)
/tests          (unit + integration, фикстуры отчётов)
```

## 14. Качество и процесс

- Каждая схема — с JSON Schema + валидация при загрузке.
- Юнит-тесты: парсеры пресетов, версии/фингерпринты, скоринг тестов.
- Интеграционные: запуск winws2 с фейковым конфигом, откат версии, импорт 3 источников.
- CI: `cargo test`, `cargo clippy -D warnings`, `npm test`, сборка NSIS.
- Никаких «магических» ручных шагов: всё через манифесты и миграции.
- Изменения поведения = правка JSON-манифеста/пресета, а не кода (по возможности).

## 15. Что вернуть в ответ

1. План реализации по этапам с оценками.
2. Скелет проекта (папки, конфиги, сборка).
3. Схемы JSON (component/strategy/profile/result) + примеры.
4. Прототип M0 (служба + UI + Пуск/Стоп).
5. Список открытых вопросов, если что-то требует уточнения.
