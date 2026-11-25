# Manka для Windows

[![Release](https://github.com/ivalushin0/Manka-UI/actions/workflows/release.yml/badge.svg)](https://github.com/ivalushin0/Manka-UI/actions/workflows/release.yml)
[![CI](https://github.com/ivalushin0/Manka-UI/actions/workflows/ci.yml/badge.svg)](https://github.com/ivalushin0/Manka-UI/actions/workflows/ci.yml)
[![Downloads](https://img.shields.io/github/downloads/ivalushin0/Manka-UI/total?label=скачивания)](https://github.com/ivalushin0/Manka-UI/releases)
[![Stars](https://img.shields.io/github/stars/ivalushin0/Manka-UI?label=%E2%98%85)](https://github.com/ivalushin0/Manka-UI/stargazers)

[Скачать последнюю версию](https://github.com/ivalushin0/Manka-UI/releases/latest) · [English](README_EN.md)

Обход DPI для Windows — десктопная версия [Manka для Android](https://github.com/ivalushin0/manka).
Один интерфейс для **Zapret 2 / Zapret**, **TG WS Proxy** и **автоподбора стратегий**.
Приложение ничего не подменяет в сети само: оно управляет официальными движками
(`winws2.exe`, `winws.exe`) и прокси `tg-ws-proxy-rs`.

> **Скачивайте только с GitHub.** У проекта нет других сайтов, зеркал на файлообменниках и
> «репаков». Если Manka предлагают скачать в другом месте — это не мы, внутри может быть что угодно.

## Возможности

- **Обход DPI**: запуск `winws2`/`winws` от имени администратора, статус, uptime, логи.
- **Сторож стратегии**: фоновая проверка, что активная стратегия всё ещё открывает сайты;
  при деградации («вчера работало — сегодня нет») сама запускает переподбор и применяет лучшую.
- **Диагностика («Доктор»)**: проверка одной кнопкой — права, конфликты с другими обходами
  (GoodbyeDPI и т.п.), драйвер WinDivert, целостность бинарников, занятые порты, автозапуск;
  экспорт архива для прикрепления к issue (секреты вырезаются).
- **Компоненты как плагины**: официальный бандл [bol-van/zapret-win-bundle](https://github.com/bol-van/zapret-win-bundle)
  и [tg-ws-proxy-rs](https://github.com/valnesfjord/tg-ws-proxy-rs) ставятся и обновляются из первоисточников,
  с **проверкой SHA256** и возможностью **отката** на предыдущую версию.
- **Единый каталог стратегий**: встроенные Windows-стратегии + наборы из
  [CDPIUI-Store](https://github.com/Storik4pro/CDPIUI-Store) (`IC:`/`UC:`-пресеты, переменные, game filter).
- **Автоподбор**: проверка TLS/HTTP/64 КБ тела по целям (YouTube, Discord, Telegram, Instagram, TikTok,
  Cloudflare и свои сайты), ранжирование по успеху и задержке, сохранение лучшей стратегии.
- **Режимы обхода**: «по списку» (только заблокированные домены — банки, игры и Госуслуги идут
  напрямую) или «весь трафик».
- **Списки доменов**: встроенный список + автообновление внешних списков ([Re-filter](https://github.com/1andrevich/Re-filter-lists))
  с зеркалами и своими источниками; `manka-exclude.txt` защищает российские сервисы, банки,
  AI-сайты и удалённый доступ от лишней подмены TLS.
- **Telegram**: локальный MTProto-прокси на `127.0.0.1:1443` без трея, ссылка `tg://`, проверка Cloudflare-маршрутов.
- **Трей**: одно окно, закрытие в трей, серая иконка при неработающем обходе, меню
  «Открыть / Обход интернета / Обход Telegram / Пауза / Выход»; при запуске Windows окно можно
  не показывать — Manka стартует сразу в трей.
- **Автозапуск**: задача планировщика с максимальными правами — запуск при входе без UAC.
- **Интерфейс на трёх языках**: русский, English, українська.
- **Журнал**: события приложения, вывод движка и прокси.

## Безопасность

Manka работает от имени администратора (так требует драйвер WinDivert), поэтому к безопасности
приложение относится параноидально:

- **Проверка загрузок.** TG WS Proxy и наборы стратегий проверяются по SHA256-дайджесту, который
  GitHub публикует для каждого ассета; несовпадение = установка отменяется, файл удаляется.
  Бандл Zapret скачивается по **иммутабельному URL конкретного коммита** (не по плавающей ветке),
  его хэш фиксируется при установке.
- **Контроль целостности.** Хэш установленных `winws2.exe`/`tg-ws-proxy.exe` записывается при
  установке; «Доктор» сверяет его повторно и заметит подмену файлов после установки.
- **CSP и валидация IPC.** В WebView включён строгий Content-Security-Policy, все команды
  бэкенда валидируют вход (идентификаторы наборов, шаблоны стратегий, схемы URL).
- **Обновления приложения** подписаны (minisign через tauri-plugin-updater); приватный ключ
  не покидает машину разработчика и GitHub Secrets.
- **Атомарная запись настроек** (`MoveFileEx` с `WRITE_THROUGH`): сбой питания не оставит
  обрезанный settings.json; повреждённый файл бэкапится, а не затирается молча.

### Антивирус ругается — это нормально?

WinDivert (драйвер перехвата пакетов) и `winws.exe` исторически детектируются некоторыми
антивирусами как «HackTool/Riskware» — так же реагируют на GoodbyeDPI, zapret и их GUI.
Manka скачивает эти файлы только из официальных репозиториев и проверяет контрольные суммы;
проверить можно и самостоятельно: ссылки на VirusTotal есть на странице каждого релиза компонентов.
Если сомневаетесь — соберите Manka из исходников (см. ниже) и сравните хэши.

## Быстрый старт (из исходников)

Требуется: Windows 10/11 x64, Rust (MSVC), Node.js 20+, WebView2, Visual Studio Build Tools + Windows SDK.

```powershell
npm install
cd app; npm install; cd ..

# установить компоненты и активировать встроенную стратегию
cargo run -p manka-core --example setup

# запуск приложения
.\node_modules\.bin\tauri.cmd dev
```

Если папка проекта синхронизируется облаком (OneDrive, pCloud и т.п.), задайте каталог сборки
вне синхронизации, иначе облачные «заглушки» могут ломать сборку:

```powershell
$env:CARGO_TARGET_DIR="$env:LOCALAPPDATA\Temp\manka-target"
```

Сборка установщика:

```powershell
.\node_modules\.bin\tauri.cmd build
# target\release\bundle\nsis\Manka_0.4.0_x64-setup.exe
# target\release\bundle\nsis\latest.json (манифест обновлений)
```

Установщик ставит приложение в `%LOCALAPPDATA%\Manka` (ярлык «Manka» в меню «Пуск»),
а данные — отдельно, в `%LOCALAPPDATA%\MankaData`; удаление приложения не трогает профили,
наборы и компоненты.

Запускайте Manka **от имени администратора** (или нажмите «Перезапустить от администратора»
в жёлтом баннере): WinDivert без этого не загружается.

## Где лежат данные

`%LOCALAPPDATA%\MankaData`:

```
settings.json        настройки
presets.json         свои и авто-стратегии
components\          zapret\<version>, tgws\<tag>, current.json
kits\<store_id>\     распакованные наборы CDPI
lists\               hostlist/ipset (включая manka-exclude.txt)
logs\                app.log, engine.log, tgws.log
results\history.json история автоподборов
store-index\         снимок CDPIUI-Store
```

`%LOCALAPPDATA%\Manka` — это каталог установки (exe и uninstaller), не данные.

Для переноса на другой ПК достаточно скопировать папку целиком; путь можно переопределить
переменной `MANKA_DATA_DIR`. Старые данные из `%LOCALAPPDATA%\Manka` переносятся автоматически
при первом запуске (эта папка зарезервирована установщиком).

## Обновления через GitHub

Manka умеет проверять и устанавливать обновления из GitHub Releases (плагин `tauri-plugin-updater`).

1. Создайте **публичный** репозиторий и запушьте туда проект.
2. В приложении: **Настройки → Обновления Manka** → репозиторий уже заполнен как
   `ivalushin0/Manka-UI`; при форке укажите свой в формате `owner/repo`.
3. В настройках репозитория на GitHub добавьте секреты Actions:
   - `TAURI_SIGNING_PRIVATE_KEY` — содержимое файла `%USERPROFILE%\.manka\updater.key`;
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — оставьте пустым, если ключ без пароля.
4. Поднимите версию в `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` и `app/package.json`,
   затем создайте тег и запушьте его:
   ```powershell
   git tag v0.2.0
   git push origin v0.2.0
   ```
5. Workflow `.github/workflows/release.yml` соберёт NSIS-установщик, подпишет его и создаст
   черновик релиза с `latest.json`. Опубликуйте релиз — после этого кнопка «Проверить обновления»
   в приложении увидит новую версию и сможет установить её с перезапуском.

Публичный ключ обновлений уже прописан в `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).
Приватный ключ `%USERPROFILE%\.manka\updater.key` **не коммитьте**. Если ключ потерян, обновления
перестанут проходить: потребуется сгенерировать новую пару и заменить `pubkey` в конфиге.

## Структура репозитория

```
app/                     Svelte 5 + Vite UI (i18n: ru/en/uk)
src-tauri/               Tauri v2 shell: команды, трей, автозапуск, сторож
crates/manka-core/       модели, провайдеры компонентов, каталог стратегий, runner, пробы,
                         автоподбор, диагностика (doctor), внешние списки, сторож
crates/manka-core/tests  smoke-тест с реальными загрузками
crates/manka-core/examples setup, autoselect_smoke
docs/                    архитектура (в т.ч. ARCHITECTURE-PROPOSAL.md — исходный дизайн-док),
                         форматы данных
manka APK/               локальная копия Android-версии (отдельный репозиторий, в git не входит)
```

## CI/CD

- **CI** (`.github/workflows/ci.yml`): на каждый push/PR — `cargo fmt --check`,
  `clippy -D warnings`, тесты ядра, `svelte-check`, сборка фронтенда; еженедельно —
  `cargo audit` / `npm audit`. Все actions запинены по SHA.
- **Release** (`.github/workflows/release.yml`): по тегу `v*` собирает NSIS-установщик,
  подписывает артефакты обновлений и создаёт черновик релиза с `latest.json`.

## Ограничения текущей версии

- Нет службы `SYSTEM`: движок запускается из процесса приложения (нужен один UAC при старте,
  если приложение не запущено планировщиком).
- Windows-порт не управляет DNS/hosts, исключениями приложений и «прокси для приложений» (Xray) —
  это следующие этапы после службы.
- Кроссплатформенность не проверялась: код Windows-first.

Подробности — в [docs/DESKTOP.md](docs/DESKTOP.md) и [docs/DATA.md](docs/DATA.md).

## Лицензии

Код Manka — MIT. Компоненты распространяются отдельно: zapret/zapret2 (MIT),
tg-ws-proxy-rs (MIT), CDPIUI-Store и наборы — по лицензиям их авторов.
