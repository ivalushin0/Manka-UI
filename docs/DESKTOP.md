# Manka для Windows — архитектура и состояние

Дата: 21.11.2026. Версия: 0.1.0 (MVP).

## 1. Что реализовано

| Блок | Состояние |
|---|---|
| Tauri v2 + Svelte 5 UI, 7 экранов | ✅ |
| Установка/обновление/откат Zapret (winws2 + winws) | ✅ |
| Установка/обновление tg-ws-proxy-rs | ✅ |
| Каталог встроенных стратегий (Windows) | ✅ |
| Импорт наборов CDPIUI-Store (`IC:`/`UC:`) | ✅ |
| Рендер аргументов winws/winws2, переменные, опции, `$LOCALCONDITION` | ✅ |
| Запуск/останов движка, лог, статус, uptime | ✅ |
| TG WS Proxy: headless, ссылка `tg://`, проверка `--check` | ✅ |
| Конфликт порта: определение чужого прокси и запуск вместо него | ✅ |
| Автоподбор с прогрессом, отменой, историей | ✅ |
| Трей, закрытие в трей, автозапуск (schtasks HIGHEST) | ✅ |
| Установщик NSIS, отдельные папки установки и данных | ✅ |
| Служба `SYSTEM`, portable-режим, подпись манифестов | ⏳ следующий этап |
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

- **Zapret** ← `https://codeload.github.com/bol-van/zapret-win-bundle/zip/refs/heads/master`,
  из архива берётся папка `zapret-winws/`. Версия — SHA коммита master (12 символов).
  Установка в `components/zapret/<sha>`, переключение `current.json`, хранение предыдущей версии для отката.
- **TG WS Proxy** ← последний релиз `valnesfjord/tg-ws-proxy-rs`, ассет
  `tg-ws-proxy-x86_64-pc-windows-gnu.zip` (fallback — win7-сборка). Установка в `components/tgws/<tag>`.
- Повторная установка с той же версией не выполняется (сравнение `current.json`).

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

См. `docs/DATA.md`. Все записи — атомарные (temp + rename), JSON в UTF-8.
Данные лежат в `%LOCALAPPDATA%\MankaData`; `%LOCALAPPDATA%\Manka` — каталог установки NSIS.

## 8. Следующие шаги

1. **Служба `SYSTEM`** с named pipe IPC и токеном; UI без прав администратора;
   установка службы NSIS-хуком; portable-режим с UAC.
2. **Обновления приложения** (Tauri updater) и подпись манифестов Ed25519.
3. **DNS/hosts** и исключения приложений по аналогии с мобильной версией (WinDivert-фильтры по процессам).
4. **Импорт пресетов zapretgui** (`builtin/winws1|winws2/*.txt`) напрямую, с проверкой
   referenced `bin/`/`lists/`.
5. **ByeDPI для Windows** как третий движок и SOCKS-режим тестирования.
6. Серверная матрица, экспорт отчётов, «условные задачи», авто-перепроверка по расписанию.
