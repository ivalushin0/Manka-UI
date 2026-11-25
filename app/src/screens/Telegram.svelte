<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import Modal from "../lib/Modal.svelte";
  import { store } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { copyText, fmtUptime } from "../lib/utils";
  import { t } from "../lib/i18n.svelte";

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
    else store.fail(t("common.copyFailed"));
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
      <div class="page-sub">{t("telegram.subtitle")}</div>
    </div>
    <span class="badge" class:ok={running} class:muted={!running}>
      {running ? `${t("common.running")} · ${fmtUptime(store.data?.proxy.uptime_sec ?? 0)}` : t("telegram.stopped")}
    </span>
  </div>

  {#if !installed}
    <div class="banner">
      <Icon name="download" size={20} />
      <span class="grow">{t("telegram.notInstalled")}</span>
      <button class="btn small primary" onclick={() => store.installComponent("tgws")} disabled={store.busy}>
        {t("common.install")}
      </button>
    </div>
  {/if}

  {#if store.portOwner && !running}
    <div class="banner bad">
      <Icon name="bolt" size={20} />
      <span class="grow">
        {t("telegram.portBusy", { port: settings?.tgws_port ?? "" })} <b>{store.portOwner.name}</b> (PID {store.portOwner.pid}).
        {#if store.isKnownProxy(store.portOwner.name)}
          {t("telegram.portBusyKnown")}
        {:else}
          {t("telegram.portBusyFree")}
        {/if}
      </span>
      {#if store.isKnownProxy(store.portOwner.name)}
        <button class="btn small danger" onclick={() => (confirmAction = "take")} disabled={store.busy}>{t("telegram.takePort")}</button>
      {/if}
    </div>
  {/if}

  <div class="card">
    <div class="card-title"><Icon name="send" size={18} /> {t("telegram.connection")}</div>
    <div class="row">
      <button class="btn" class:primary={!running} class:danger={running} onclick={() => store.toggleProxy()} disabled={store.busy || !installed}>
        <Icon name={running ? "stop" : "play"} size={15} /> {running ? t("telegram.stopProxy") : t("telegram.startProxy")}
      </button>
      <button class="btn" onclick={() => store.checkProxy()} disabled={store.tgwsChecking}>
        <Icon name="bolt" size={15} /> {store.tgwsChecking ? t("common.checking") : t("common.check")}
      </button>
      <button class="btn" onclick={open}><Icon name="external" size={15} /> {t("telegram.openInTelegram")}</button>
    </div>

    <hr class="sep" style="margin:16px 0" />

    <div class="kv">
      <span class="k">{t("telegram.server")}</span><span class="mono">127.0.0.1</span>
      <span class="k">{t("telegram.port")}</span><span class="mono">{settings?.tgws_port}</span>
      <span class="k">{t("telegram.secret")}</span>
      <span class="row">
        <span class="mono wrap grow">{showSecret ? settings?.tgws_secret : "••••••••••••••••••••••••••••••••"}</span>
        <button class="btn small ghost" onclick={() => (showSecret = !showSecret)}>{showSecret ? t("common.hide") : t("common.show")}</button>
        <button class="btn small ghost" onclick={() => copy(settings?.tgws_secret ?? "", t("telegram.secretCopied"))}>
          <Icon name="copy" size={14} />
        </button>
        <button
          class="btn small danger"
          onclick={() => (confirmAction = "regen")}
        >
          {t("telegram.newSecret")}
        </button>
      </span>
      <span class="k">{t("telegram.link")}</span>
      <span class="row">
        <span class="mono wrap grow">{link}</span>
        <button class="btn small ghost" onclick={() => copy(link, t("common.linkCopied"))}><Icon name="copy" size={14} /></button>
      </span>
    </div>

    {#if store.tgwsCheck}
      <div class="banner" class:bad={!store.tgwsCheck.ok} style="margin-top:14px;display:block">
        <div class="row" style="margin-bottom:6px">
          <b>{store.tgwsCheck.ok ? t("telegram.checkOk") : t("telegram.checkFailed")}</b>
        </div>
        {#if store.tgwsCheck.output}
          <pre class="mono wrap" style="margin:6px 0 0;max-height:180px;overflow:auto">{store.tgwsCheck.output}</pre>
        {/if}
      </div>
    {/if}
  </div>

  <div class="card">
    <div class="card-title"><Icon name="sliders" size={17} /> {t("telegram.settings")}</div>
    <div class="grid-2">
      <label class="field">
        {t("telegram.listenPort")}
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
        {t("telegram.poolSize")}
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
      {t("telegram.cloudflare")}
    </label>

    <label class="field" style="margin-top:14px">
      {t("telegram.dcIps")}
      <textarea
        placeholder="2:149.154.167.220, 4:149.154.167.220"
        value={settings?.tgws_dc_ips ?? ""}
        onchange={(e) => store.saveSettings({ tgws_dc_ips: (e.target as HTMLTextAreaElement).value })}
      ></textarea>
    </label>

    <label class="field" style="margin-top:14px">
      {t("telegram.extraArgs")}
      <input
        type="text"
        placeholder={t("telegram.extraPh")}
        value={settings?.tgws_extra ?? ""}
        onchange={(e) => store.saveSettings({ tgws_extra: (e.target as HTMLInputElement).value })}
      />
    </label>

    <div class="small dim" style="margin-top:12px;line-height:1.55">
      {t("telegram.note")}
    </div>
  </div>
</div>

{#if confirmAction === "take"}
  <Modal
    title={t("telegram.freePortTitle")}
    subtitle={`${store.portOwner?.name ?? t("telegram.process")} · PID ${store.portOwner?.pid ?? "—"}`}
    onclose={() => (confirmAction = null)}
  >
    <div class="small muted" style="line-height:1.55">
      {t("telegram.freePortText", { port: settings?.tgws_port ?? "" })}
    </div>
    <div class="row" style="justify-content:flex-end;margin-top:16px">
      <button class="btn" onclick={() => (confirmAction = null)}>{t("common.cancel")}</button>
      <button class="btn danger" onclick={takePort} disabled={store.busy}>{t("telegram.takePort")}</button>
    </div>
  </Modal>
{/if}

{#if confirmAction === "regen"}
  <Modal
    title={t("telegram.regenTitle")}
    subtitle={t("telegram.regenSubtitle")}
    onclose={() => (confirmAction = null)}
  >
    <div class="small muted" style="line-height:1.55">
      {t("telegram.regenText")}
    </div>
    <div class="row" style="justify-content:flex-end;margin-top:16px">
      <button class="btn" onclick={() => (confirmAction = null)}>{t("common.cancel")}</button>
      <button
        class="btn danger"
        onclick={() => {
          confirmAction = null;
          void store.regenerateSecret();
        }}
      >
        {t("telegram.regen")}
      </button>
    </div>
  </Modal>
{/if}
