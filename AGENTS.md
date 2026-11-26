# Правила работы над Manka

Этот файл — обязательный чек-лист для людей и агентов, которые меняют проект.

## 1. Всё комментируем

- **Релиз без комментария не выпускаем.** У каждого релиза на GitHub должны быть
  человеческие release notes: что исправлено, что добавлено, как обновиться и какие есть
  ограничения. Шаблонная строка «сборка версии X» комментарием не считается.
- Заметки хранятся в `.github/release-notes/<tag>.md` (например, `v0.4.2.md`) и
  подставляются в черновик релиза workflow. Если файла нет — сборка релиза падает.
- В коде комментарий объясняет **почему**, а не «что»: неочевидные решения, обходные пути,
  причины исключений в списках и особенности поведения Windows.
- В коммите заголовок = версия и суть (`0.4.2: fix YouTube sign-in`), тело = причина,
  эффект и миграция для существующих установок.
- Если меняется поведение, списки, стратегии или формат данных — обновите `docs/`:
  `docs/LISTS.md`, `docs/DESKTOP.md`, `docs/DATA.md`.
- Скриншоты для README — в `docs/screenshots/`. Секреты, ключи, `tg://`-ссылки и токены
  на скриншотах обязательно замазываются.

## 2. Версии и релиз

1. Поднять версию во всех местах:
   - `package.json`
   - `app/package.json`
   - `src-tauri/tauri.conf.json`
   - `src-tauri/Cargo.toml`
   - `crates/manka-core/Cargo.toml`
   - `Cargo.lock` обновится при сборке.
2. Проверки перед тегом:
   ```powershell
   npm --prefix app run check
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test -p manka-core
   ```
3. Собрать установщик:
   ```powershell
   $env:CARGO_TARGET_DIR="$env:LOCALAPPDATA\Temp\manka-build-target"
   $env:TAURI_SIGNING_PRIVATE_KEY="$env:USERPROFILE\.manka\updater.key"
   $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD=''
   npm run tauri build -- --bundles nsis
   ```
4. Создать `.github/release-notes/v<version>.md` с заметками.
5. Закоммитить, запушить `main`, создать и запушить тег:
   ```powershell
   git add -A
   git commit -m "<version>: <суть>"
   git push origin main
   git tag v<version>
   git push origin v<version>
   ```
6. Дождаться workflow **Release**, проверить ассеты (`latest.json`, установщик, `.sig`)
   и опубликовать черновик:
   ```powershell
   gh release edit v<version> --draft=false
   ```
7. Скопировать установщик в папку для второго ПК:
   `D:\Работа Nordfox\Работа\a.Личное\Manka-UI\`.

## 3. Списки доменов

- `manka-hostlist.txt` — что **обходить**; `manka-exclude.txt` — что **не трогать**.
- `--hostlist-exclude` имеет приоритет над `--hostlist`. Если домен попал в оба списка,
  обход к нему не применяется.
- `accounts.youtube.com` должен быть в hostlist и **не** в exclude: через него завершается
  вход в Google-аккаунт на YouTube, и провайдер его режет.
- Не добавляйте в exclude домены, которые провайдер блокирует: исключение = «работает без
  обхода», а не «не трогать вообще».
- После правок списков проверьте вживую: `curl.exe -sS -o NUL -w "%{http_code}" <url>`.
- Подробности и источники — в `docs/LISTS.md`.

## 4. Стиль UI

- Главный экран должен оставаться компактным: hero, текущий профиль, тонкая плашка
  Telegram-прокси. Не возвращайте блок «Установленные движки DPI».
- Уведомления об обновлении: тост один раз на версию + «горящая» карточка на главном экране.
- Интерфейс на трёх языках: `app/src/lib/i18n.svelte.ts` (`ru`, `en`, `uk`). Новые ключи
  добавляются во все три словаря.
- Проверять `npm --prefix app run check` и вёрстку при 780×520 и 1180×800.

## 5. Безопасность

- Не коммитить `%USERPROFILE%\.manka\updater.key`, токены, секреты прокси и реальные
  `tg://`-ссылки.
- Скачиваемые компоненты проверяются по SHA256; не ослабляйте проверки.
- Команды бэкенда валидируют вход; не добавляйте команды без валидации.
