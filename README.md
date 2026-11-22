# Manka для Windows

Обход DPI для Windows — десктопная версия [Manka для Android](https://github.com/ivalushin0/manka).
Один интерфейс для **Zapret 2 / Zapret**, **TG WS Proxy** и **автоподбора стратегий**.
Приложение ничего не подменяет в сети само: оно управляет официальными движками
(`winws2.exe`, `winws.exe`) и прокси `tg-ws-proxy-rs`.

## Возможности

- **Обход DPI**: запуск `winws2`/`winws` от имени администратора, статус, uptime, логи.
- **Компоненты как плагины**: официальный бандл [bol-van/zapret-win-bundle](https://github.com/bol-van/zapret-win-bundle)
  и [tg-ws-proxy-rs](https://github.com/valnesfjord/tg-ws-proxy-rs) ставятся и обновляются из первоисточников.
- **Единый каталог стратегий**: встроенные Windows-стратегии + наборы из
  [CDPIUI-Store](https://github.com/Storik4pro/CDPIUI-Store) (`IC:`/`UC:`-пресеты, переменные, game filter).
- **Автоподбор**: проверка TLS/HTTP/64 КБ тела по целям (YouTube, Discord, Telegram, Instagram, TikTok,
  Cloudflare и свои сайты), ранжирование по успеху и задержке, сохранение лучшей стратегии.
- **Telegram**: локальный MTProto-прокси на `127.0.0.1:1443` без трея, ссылка `tg://`, проверка Cloudflare-маршрутов.
- **Трей**: одно окно, закрытие в трей, серая иконка при неработающем обходе, меню
  «Открыть / Обход интернета / Обход Telegram / Пауза / Выход».
- **Автозапуск**: задача планировщика с максимальными правами — запуск при входе без UAC.
- **Журнал**: события приложения, вывод движка и прокси.

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
# target\release\bundle\nsis\Manka_0.2.1_x64-setup.exe
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
2. В приложении: **Настройки → Обновления Manka** → укажите репозиторий в формате `owner/repo`.
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
app/                     Svelte 5 + Vite UI
src-tauri/               Tauri v2 shell: команды, трей, автозапуск
crates/manka-core/       модели, провайдеры компонентов, каталог стратегий, runner, пробы, автоподбор
crates/manka-core/tests  smoke-тест с реальными загрузками
crates/manka-core/examples setup, autoselect_smoke
docs/                    архитектура, форматы данных
manka APK/               локальная копия Android-версии (отдельный репозиторий, в git не входит)
```

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
