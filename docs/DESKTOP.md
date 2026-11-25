# Manka для Windows — архитектура и состояние

Актуализировано 25.11.2026 под версию 0.4.0.

## 1. Что реализовано

| Блок | Состояние |
|---|---|
| Tauri v2 + Svelte 5 UI, 8 экранов, i18n (ru/en/uk) | ✅ |
| Установка/обновление/откат Zapret (winws2 + winws) | ✅ |
| Установка/обновление/откат tg-ws-proxy-rs | ✅ |
| Верификация загрузок: SHA256-дайджесты GitHub, пиннинг Zapret по коммиту | ✅ |
| Контроль целостности установленных бинарников (хэш при установке → сверка в «Докторе») | ✅ |
| Каталог встроенных стратегий (Windows) | ✅ |
| Импорт наборов CDPIUI-Store (`IC:`/`UC:`) | ✅ |
| Рендер аргументов winws/winws2, переменные, опции, `$LOCALCONDITION` | ✅ |
| Режимы обхода «по списку» / «весь трафик» (`bypass_scope`) | ✅ |
| Запуск/останов движка, лог, статус, uptime | ✅ |
| TG WS Proxy: headless, ссылка `tg://`, проверка `--check` | ✅ |
| Конфликт порта: определение чужого прокси и запуск вместо него | ✅ |
| Автоподбор с прогрессом, отменой, историей | ✅ |
| Сторож стратегии (watchdog): фоновая проверка + автопереподбор | ✅ |
| Внешние списки доменов (Re-filter + зеркала + свои URL), автообновление | ✅ |
| «Доктор»: диагностика в один клик + экспорт zip для issue | ✅ |
| Трей, закрытие в трей, автозапуск (schtasks HIGHEST) | ✅ |
| CSP в WebView, валидация IPC-команд, атомарная запись настроек | ✅ |
| Установщик NSIS, отдельные папки установки и данных | ✅ |
| CI: fmt/clippy/tests/svelte-check/audit, actions запинены по SHA | ✅ |
| Служба `SYSTEM`, portable-режим | ⏳ следующий этап |
| DNS/hosts, исключения приложений, Xray «прокси для приложений» | ⏳ |
| ByeDPI на Windows | ⏳ (нет движения в MVP) |

## 2. Процессы

```
manka.exe (Tauri, UI + трей)
 ├─ winws2.exe / winws.exe   (WinDivert, права администратора)
 └─ tg-ws-proxy.exe          (127.0.0.1:1443, без трея)
```

`manka.exe` — единственный владелец дочерних процессов. При выходе через трей или закрытии
окна без «в трей» процессы останавливаются. При закрытии в трей движок продолжает работать.
При старте, если `settings.enabled = true`, движок поднимается автоматически.

Почему без службы в MVP: служба `SYSTEM` — отдельный этап. Сейчас используется планировщик
Windows с `HIGHEST`: автозапуск при входе проходит без UAC, а ручной запуск требует одного
подтверждения (или «Перезапустить от администратора»).

## 3. Компоненты

`crates/manka-core/src/components.rs`:

- **Zapret** ← `https://api.github.com/repos/bol-van/zapret-win-bundle/commits/master` (резолв
  полного SHA), затем скачивание по **иммутабельному URL** `codeload.../zip/<полный SHA>` —
  метка версии (12 символов SHA) всегда соответствует содержимому. Из архива берётся папка
  `zapret-winws/`. Установка в `components/zapret/<sha>`, переключение `current.json`,
  хранение предыдущей версии для отката.
- **TG WS Proxy** ← последний релиз `valnesfjord/tg-ws-proxy-rs`, ассет
  `tg-ws-proxy-x86_64-pc-windows-gnu.zip` (fallback — win7-сборка). Перед распаковкой
  **обязательная проверка SHA256** по полю `digest` из GitHub API; без дайджеста установка
  отменяется. Установка в `components/tgws/<tag>`, откат поддержан.
- Наборы CDPI (store.rs) — та же проверка дайджеста для zip-архивов.
- При установке записывается SHA256 главного exe (`current.json: exe_sha256`); «Доктор»
  сверяет его повторно (`verify_installed`) и заметит подмену файлов после установки.
- Повторная установка с той же версией не выполняется (сравнение `current.json`).
- Лимиты: скачивание ≤ 1 ГБ, распаковка ≤ 2 ГБ (zip-бомбы), zip-slip-гвард на путях.

## 3.1 Сторож, списки, «Доктор»

- **Сторож** (`watchdog.rs` + фоновый цикл в shell): раз в `watchdog_interval_min` минут
  прогоняет 1 запрос по ≤6 целям через активный движок. При успехе <50% проверяет контрольные
  URL (gstatic/ya.ru): если и они мертвы — виновата сеть, ничего не делает; иначе при
  `watchdog_auto_fix` запускает быстрый автоподбор (под общим `auto_lock` с ручным подбором)
  и применяет лучшую стратегию, события `watchdog` / `watchdog-fixed` / `watchdog-failed`.
- **Списки** (`lists.rs`): источники по умолчанию (Re-filter, raw.githubusercontent +
  зеркало jsDelivr) + пользовательские https-URL. Скачанное нормализуется (комментарии,
  `*.`/ `+.` префиксы, мусор отбрасываются), мержится со встроенным списком в
  `lists/effective-hostlist.txt` — именно он подставляется в `--hostlist=`. Режим
  `bypass_scope=all` отключает инъекцию hostlist (десинк на весь TLS/HTTP).
- **«Доктор»** (`doctor.rs`): права, установка движка, целостность бинарников, чужие
  DPI-процессы (GoodbyeDPI/ByeDPI/чужие winws), драйвер WinDivert (`sc query`), владелец
  порта прокси, папка данных в облачной синхронизации, консистентность автозапуска, списки.
  `export` собирает zip (doctor.txt + логи ≤512 КБ + settings.json с вырезанным секретом +
  current.json компонентов) в `results/manka-diagnostics-<дата>.zip`.

## 4. Стратегии

Нормализованная модель (`Strategy`): `engine`, `template`, `kit_id`, `variables`, `options`,
`flags`, `tcp_ports`, `udp_ports`, `source`, `tags`.

Рендер (`crates/manka-core/src/args.rs`):

1. `$LOCALCONDITION(flag==true ? a : b)` по значениям `jparams` набора;
2. подстановка `%NAME%` (переменные набора + выбранные значения опций, до 5 проходов);
3. `$GETCURRENTDIR()`/`%~dp0` → папка набора, `$KIT`, `$ENGINE`, `$LUA`, `$FAKE`,
   `$LISTS`, `$EXCLUDE`, `{sni}`;
4. разбиение командной строки с учётом кавычек (обратные слэши путей не трогаются);
5. для `winws2` `--wf-tcp/--wf-udp` переводятся в `--wf-tcp-out/--wf-udp-out`
   (winws2 не принимает legacy-форму), при отсутствии WinDivert-фильтра добавляется `--wf-tcp-out=80,443`;
6. для `winws2` при отсутствии `--lua-init` подключаются `lua/zapret-lib.lua` и `lua/zapret-antidpi.lua`;
7. удаляются Android-only опции (`--ssid-filter`, `--nlm-filter`) и пустые профили `--new`.

Встроенные стратегии: 12 штук (7 для zapret2, 5 для legacy) + каталог быстрых (20–25) и полных
кандидатов. Каталог совместим с подходами мобильной Manka (fake/multisplit/multidisorder/hostfakesplit,
fake TLS из файла для legacy).

## 5. Магазин CDPI

`refresh_index` скачивает `codeload.github.com/Storik4pro/CDPIUI-Store/zip/refs/heads/main`
и сохраняет JSON/MD. `items()` разбирает категории (`configs`, `subscriptions`, ...).
Установка набора:

- релиз репозитория из `version_control_link` (или прямая ссылка для `lsubscription`);
- архив распаковывается в `kits/<store_id>`, пишется `manka-kit.json` с версией;
- JSON-пресеты (`meta` = `IC:`/`UC:`) переводятся в `Strategy`; target
  `CSZTBN012` → winws, `CSZTBN062` → winws2, `CSBIHA024` → ByeDPI (пропускается);
- локализация `$LOADSTRING(...)`, `commaVars`, `availableCommaVarsValues`, `jparams`.

Проверено на реальных наборах: YtDisBystro 3.4, zapret-discord-youtube 1.10.3,
zapret-discord-youtube-alt 1.9.1 — установка и рендер аргументов.

## 6. Автоподбор

`crates/manka-core/src/autoselect.rs`:

1. кандидаты: активная стратегия + наборы магазина (если включено) + каталог (быстрый/полный),
   дедупликация по `engine::template`;
2. основной движок останавливается, снимается baseline без обхода;
3. каждый кандидат: рендер → запуск winws → ожидание 0.8–1.6 с → пробы → остановка;
4. пробы: TLS + HTTP + первые 64 КБ, ошибки обрезки/таймаута; 8 параллельных целей,
   несколько запросов на цель;
5. балл: процент успеха, затем средняя задержка; при 100% N раз подряд движок пропускается;
6. прогресс идёт событиями `auto-progress`, отмена — `auto_cancel`;
7. результат сохраняется в `presets.json` (source `auto`), применяется как активная стратегия,
   история — `results/history.json`.

## 7. Данные и файлы

См. `docs/DATA.md`. Все записи — атомарные (`MoveFileExW` с `REPLACE_EXISTING|WRITE_THROUGH`),
JSON в UTF-8; повреждённый `settings.json` бэкапится как `settings.corrupt-*.json`, а не
затирается молча. Данные лежат в `%LOCALAPPDATA%\MankaData`; `%LOCALAPPDATA%\Manka` — каталог
установки NSIS.

## 8. Следующие шаги

1. **Служба `SYSTEM`** с named pipe IPC и токеном; UI без прав администратора;
   установка службы NSIS-хуком; portable-режим с UAC. Дизайн — `docs/ARCHITECTURE-PROPOSAL.md`.
2. **Разделение трафика по приложениям** (per-app split) — требует форка winws2
   (см. подход mqclass/zapret2-gui) либо WinDivert-фильтров по процессам.
3. **DNS/hosts** и DoH-переключатель.
4. **Импорт пресетов zapretgui** (`builtin/winws1|winws2/*.txt`) напрямую, с проверкой
   referenced `bin/`/`lists/`.
5. **ByeDPI для Windows** как третий движок и SOCKS-режим тестирования.
6. Кодогенерация типов TS из Rust (`specta`/`ts-rs`) вместо ручного зеркала `types.ts`.
