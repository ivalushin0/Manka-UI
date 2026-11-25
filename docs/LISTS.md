# Списки обхода

Manka использует два текстовых списка в `%LOCALAPPDATA%\MankaData\lists`:

## `manka-hostlist.txt` — что требует обхода

Домены, к которым применяется DPI-десинхронизация. Всё, чего нет в этом списке,
не трогается вовсе. Список собран и объединён из:

- `Flowseal/zapret-discord-youtube` — `lists/list-general.txt`, `lists/list-google.txt`
  (Discord, YouTube, Google, Cloudflare/DNS, медиа-сервисы);
- популярных российских наборов для zapret/GoodbyeDPI (Instagram, Facebook, X/Twitter,
  TikTok, Telegram Web, LinkedIn, Reddit, Spotify, Netflix, заблокированные медиа и
  торрент-трекеры);
- `CDPIUI-Store` и подписок CDPI, установленных в Manka.

Указывать базовый домен достаточно: zapret автоматически покрывает поддомены.
Строка, начинающаяся с `^`, означает только точный хост без поддоменов.

## `manka-exclude.txt` — что не требует обхода

Домены, которые исключены из десинхронизации. Сюда входят:

- российские банки, госуслуги, Yandex, VK, Mail.ru, Ozon, Wildberries, Avito, RZD,
  Кинопоиск и другие сервисы, которые работают без обхода и ломаются от подмены TLS;
- Microsoft, Steam, Epic Games, Riot, NVIDIA, Twitch и другие сервисы, которым
  обход не нужен;
- AI-сервисы: Kimi/Moonshot, Qwen/Tongyi, DeepSeek, ChatGPT/OpenAI, Claude, Gemini,
  Mistral, Hugging Face, Groq, OpenRouter, Perplexity, Character.ai, Midjourney,
  ElevenLabs, Suno и другие — их TLS не должен изменяться;
- AnyDesk, Radmin, Tailscale, TeamViewer, RustDesk и другие средства удалённого доступа;
- CDN-домены AI-сервисов (`volces.com`, `alicdn.com`, `aliyuncs.com` и т.п.).

`hostlist-exclude` имеет приоритет над `hostlist`: если домен попал в оба списка,
десинхронизация к нему не применяется.

## QUIC

При включённой настройке «Блокировать QUIC» исходящий QUIC Initial на UDP/443
отбрасывается, и браузер переходит на TCP. Это позволяет применять hostlist
к обычному TLS и не ломать QUIC-соединения AI-сайтов и YouTube.

## Источники

- https://github.com/Flowseal/zapret-discord-youtube
- https://github.com/bol-van/zapret
- https://github.com/Storik4pro/CDPIUI-Store
- https://github.com/ValdikSS/GoodbyeDPI

Списки можно редактировать вручную; zapret перечитывает их при изменении.
