<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import Modal from "../lib/Modal.svelte";
  import { store } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { copyText, fmtUptime } from "../lib/utils";

  const settings = $derived(store.settings);
  const running = $derived(store.data?.proxy.running ?? false);
  const installed = $derived(store.data?.components.find((c) => c.id === "tgws")?.installed ?? false);

  let showSecret = $state(false);
  let confirmAction = $state<null | "take" | "regen">(null);

  $effect(() => {
    // re-check the port whenever it changes
    settings?.tgws_port;
    store.checkPort();
  });

  async function takePort() {
    if (!settings) return;
    confirmAction = null;
    await store.takePortAndStart();
  }

  const link = $derived(
    settings ? `tg://proxy?server=127.0.0.1&port=${settings.tgws_port}&secret=dd${settings.tgws_secret}` : "",
  );

  async function copy(text: string, message: string) {
    if (await copyText(text)) store.notify(message);
    else store.fail("Не удалось скопировать");
  }

  async function open() {
    try {
      await api.openUrl(link);
    } catch (e) {
      store.fail(e);
    }
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <div class="page-title">Telegram</div>
      <div class="page-sub">Локальный MTProto-прокси через WebSocket (tg-ws-proxy-rs)</div>
    </div>
    <span class="badge" class:ok={running} class:muted={!running}>
      {running ? `работает · ${fmtUptime(store.data?.proxy.uptime_sec ?? 0)}` : "остановлен"}
    </span>
  </div>

  {#if !installed}
    <div class="banner">
      <Icon name="download" size={20} />
      <span class="grow">Прокси-компонент не установлен.</span>
      <button class="btn small primary" onclick={() => store.installComponent("tgws")} disabled={store.busy}>
        Установить
      </button>
    </div>
  {/if}

  {#if store.portOwner && !running}
    <div class="banner bad">
      <Icon name="bolt" size={20} />
      <span class="grow">
        Порт {settings?.tgws_port} занят: <b>{store.portOwner.name}</b> (PID {store.portOwner.pid}).
        {#if store.isKnownProxy(store.portOwner.name)}
          Это другой Telegram-прокси — можно остановить его и запустить Manka-прокси на том же порту.
        {:else}
          Освободите порт или выберите другой.
        {/if}
      </span>
      {#if store.isKnownProxy(store.portOwner.name)}
        <button class="btn small danger" onclick={() => (confirmAction = "take")} disabled={store.busy}>Остановить и запустить</button>
      {/if}
    </div>
  {/if}

  <div class="card">
    <div class="card-title"><Icon name="send" size={18} /> Подключение</div>
    <div class="row">
      <button class="btn" class:primary={!running} class:danger={running} onclick={() => store.toggleProxy()} disabled={store.busy || !installed}>
        <Icon name={running ? "stop" : "play"} size={15} /> {running ? "Остановить прокси" : "Запустить прокси"}
      </button>
      <button class="btn" onclick={() => store.checkProxy()} disabled={store.tgwsChecking}>
        <Icon name="bolt" size={15} /> {store.tgwsChecking ? "Проверяю…" : "Проверить"}
      </button>
      <button class="btn" onclick={open}><Icon name="external" size={15} /> Открыть в Telegram</button>
    </div>

    <hr class="sep" style="margin:16px 0" />

    <div class="kv">
      <span class="k">Сервер</span><span class="mono">127.0.0.1</span>
      <span class="k">Порт</span><span class="mono">{settings?.tgws_port}</span>
      <span class="k">Секрет</span>
      <span class="row">
        <span class="mono wrap grow">{showSecret ? settings?.tgws_secret : "••••••••••••••••••••••••••••••••"}</span>
        <button class="btn small ghost" onclick={() => (showSecret = !showSecret)}>{showSecret ? "Скрыть" : "Показать"}</button>
        <button class="btn small ghost" onclick={() => copy(settings?.tgws_secret ?? "", "Секрет скопирован")}>
          <Icon name="copy" size={14} />
        </button>
        <button
          class="btn small danger"
          onclick={() => (confirmAction = "regen")}
        >
          Новый
        </button>
      </span>
      <span class="k">Ссылка</span>
      <span class="row">
        <span class="mono wrap grow">{link}</span>
        <button class="btn small ghost" onclick={() => copy(link, "Ссылка скопирована")}><Icon name="copy" size={14} /></button>
      </span>
    </div>

    {#if store.tgwsCheck}
      <div class="banner" class:bad={!store.tgwsCheck.ok} style="margin-top:14px;display:block">
        <div class="row" style="margin-bottom:6px">
          <b>{store.tgwsCheck.ok ? "Проверка пройдена" : "Проверка не прошла"}</b>
        </div>
        {#if store.tgwsCheck.output}
          <pre class="mono wrap" style="margin:6px 0 0;max-height:180px;overflow:auto">{store.tgwsCheck.output}</pre>
        {/if}
      </div>
    {/if}
  </div>

  <div class="card">
    <div class="card-title"><Icon name="sliders" size={17} /> Настройки прокси</div>
    <div class="grid-2">
      <label class="field">
        Порт прослушивания
        <input
          id="tgws-port"
          type="number"
          min="1"
          max="65535"
          value={settings?.tgws_port}
          onchange={(e) => store.saveSettings({ tgws_port: Number((e.target as HTMLInputElement).value) })}
        />
      </label>
      <label class="field">
        Размер пула соединений
        <input
          type="number"
          min="1"
          max="16"
          value={settings?.tgws_pool_size}
          onchange={(e) => store.saveSettings({ tgws_pool_size: Number((e.target as HTMLInputElement).value) })}
        />
      </label>
    </div>

    <label class="check" style="margin-top:14px">
      <input
        type="checkbox"
        checked={settings?.tgws_cloudflare ?? true}
        onchange={(e) => store.saveSettings({ tgws_cloudflare: (e.target as HTMLInputElement).checked })}
      />
      Использовать публичные Cloudflare-домены (--default-domains)
    </label>

    <label class="field" style="margin-top:14px">
      Прямые адреса Telegram DC (формат DC:IP, по одному или через запятую)
      <textarea
        placeholder="2:149.154.167.220, 4:149.154.167.220"
        value={settings?.tgws_dc_ips ?? ""}
        onchange={(e) => store.saveSettings({ tgws_dc_ips: (e.target as HTMLTextAreaElement).value })}
      ></textarea>
    </label>

    <label class="field" style="margin-top:14px">
      Дополнительные аргументы
      <input
        type="text"
        placeholder="например: --fronting-domain=sprinthost.ru"
        value={settings?.tgws_extra ?? ""}
        onchange={(e) => store.saveSettings({ tgws_extra: (e.target as HTMLInputElement).value })}
      />
    </label>

    <div class="small dim" style="margin-top:12px;line-height:1.55">
      После изменения порта или секрета перезапустите прокси и подключите Telegram заново по ссылке.
      Если прямой адрес Telegram заблокирован, прокси сам перейдёт на Cloudflare.
    </div>
  </div>
</div>

{#if confirmAction === "take"}
  <Modal
    title="Освободить порт?"
    subtitle={`${store.portOwner?.name ?? "Процесс"} · PID ${store.portOwner?.pid ?? "—"}`}
    onclose={() => (confirmAction = null)}
  >
    <div class="small muted" style="line-height:1.55">
      Сейчас порт {settings?.tgws_port} занят другим Telegram-прокси. Manka остановит его и запустит свой
      прокси на этом же порту. Текущие подключения Telegram будут разорваны.
    </div>
    <div class="row" style="justify-content:flex-end;margin-top:16px">
      <button class="btn" onclick={() => (confirmAction = null)}>Отмена</button>
      <button class="btn danger" onclick={takePort} disabled={store.busy}>Остановить и запустить</button>
    </div>
  </Modal>
{/if}

{#if confirmAction === "regen"}
  <Modal
    title="Сгенерировать новый секрет?"
    subtitle="Telegram потребуется подключить заново"
    onclose={() => (confirmAction = null)}
  >
    <div class="small muted" style="line-height:1.55">
      Старая ссылка перестанет работать. Если прокси запущен, он будет перезапущен с новым секретом.
    </div>
    <div class="row" style="justify-content:flex-end;margin-top:16px">
      <button class="btn" onclick={() => (confirmAction = null)}>Отмена</button>
      <button
        class="btn danger"
        onclick={() => {
          confirmAction = null;
          void store.regenerateSecret();
        }}
      >
        Сгенерировать
      </button>
    </div>
  </Modal>
{/if}
