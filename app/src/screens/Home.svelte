<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { store } from "../lib/store.svelte";
  import { copyText, fmtTime, fmtUptime, percent, sourceTitle } from "../lib/utils";
  import { api } from "../lib/api";
  import { t } from "../lib/i18n.svelte";

  const settings = $derived(store.settings);
  const active = $derived(store.activeStrategy);
  const engine = $derived(store.data?.engine);
  const zapret = $derived(store.data?.components.find((c) => c.id === "zapret"));
  const tgws = $derived(store.data?.components.find((c) => c.id === "tgws"));
  const proxyOn = $derived(store.data?.proxy.running ?? false);
  const installed = $derived((store.data?.components ?? []).filter((c) => c.installed && c.kind !== "proxy"));
  const dpiInstalled = $derived(installed.length > 0);

  const powerClass = $derived(
    store.engineRunning ? "on" : settings?.enabled ? "warn" : store.elevated ? "" : "bad",
  );

  const groups = $derived.by(() => {
    const results = store.serviceResults;
    if (!results) return [];
    const selected = new Set(settings?.targets ?? []);
    return (store.data?.target_groups ?? [])
      .filter((g) => selected.has(g.id))
      .map((g) => {
        const rs = results.filter((r) => g.urls.includes(r.url));
        const ok = rs.reduce((n, r) => n + r.ok, 0);
        const total = rs.reduce((n, r) => n + r.total, 0);
        return { ...g, ok, total, pc: percent(ok, total), failed: rs.filter((r) => r.ok < r.total) };
      })
      .filter((g) => g.total > 0);
  });

  const overall = $derived.by(() => {
    const results = store.serviceResults;
    if (!results) return null;
    const ok = results.reduce((n, r) => n + r.ok, 0);
    const total = results.reduce((n, r) => n + r.total, 0);
    return { ok, total, pc: percent(ok, total) };
  });

  const tgLink = $derived(
    settings ? `tg://proxy?server=127.0.0.1&port=${settings.tgws_port}&secret=dd${settings.tgws_secret}` : "",
  );

  async function copyLink() {
    if (await copyText(tgLink)) store.notify(t("common.linkCopied"));
    else store.fail(t("home.copyLinkFailed"));
  }

  async function openLink() {
    try {
      await api.openUrl(tgLink);
    } catch (e) {
      store.fail(e);
    }
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <div class="page-title">Manka</div>
      <div class="page-sub">{t("home.subtitle")}</div>
    </div>
    <button class="btn small" onclick={() => store.refresh()} disabled={store.busy}>
      <Icon name="refresh" size={15} /> {t("common.refresh")}
    </button>
  </div>

  {#if !zapret?.installed}
    <div class="banner">
      <Icon name="package" size={20} />
      <span class="grow">{t("home.zapretNotInstalled")}</span>
      <button class="btn small primary" onclick={() => store.installComponent("zapret")} disabled={store.busy}>
        <Icon name="download" size={14} /> {t("common.install")}
      </button>
    </div>
  {/if}

  <div class="card hero">
    <button
      class="power {powerClass} {store.busy ? 'spin' : ''}"
      onclick={() => store.togglePower()}
      disabled={store.busy || (!store.elevated && !store.engineRunning)}
      title={store.elevated ? t("home.powerOn") : t("home.needAdmin")}
    >
      <Icon name="power" size={62} />
    </button>

    <div class="hero-text grow">
      <h2>
        {#if store.engineRunning}
          {t("home.stateOn")}
        {:else if store.busy}
          {t("home.stateApplying")}
        {:else if !store.elevated}
          {t("home.needAdmin")}
        {:else if settings?.enabled}
          {t("home.stateEngineStopped")}
        {:else}
          {t("home.stateOff")}
        {/if}
      </h2>
      <p>
        {#if store.engineRunning && active}
          {active.name}
        {:else if !store.elevated}
          {t("home.needAdminDetail")}
        {:else}
          {t("home.powerHint")}
        {/if}
      </p>

      {#if engine?.running}
        <div class="row small muted" style="margin-top:10px">
          <span><Icon name="clock" size={14} /> {fmtUptime(engine.uptime_sec)}</span>
          <span>PID {engine.pid}</span>
          {#if settings?.last_check_rate != null && settings.last_check_rate >= 0}
            <span>{t("home.lastCheck", { rate: settings.last_check_rate })}</span>
          {/if}
        </div>
      {/if}

      <div class="row" style="margin-top:16px">
        <button class="btn primary" onclick={() => (store.screen = "auto")}>
          <Icon name="wand" size={16} /> {t("home.pickStrategy")}
        </button>
        <button class="btn" onclick={() => (store.screen = "strategies")}>
          <Icon name="layers" size={16} /> {t("nav.strategies")}
        </button>
        <button class="btn" onclick={() => store.runDiagnostics()} disabled={store.diagnosticsBusy}>
          <Icon name="bolt" size={16} />
          {store.diagnosticsBusy ? t("common.checking") : t("nav.doctor")}
        </button>
      </div>
    </div>
  </div>

  <div class="card">
    <div class="card-title">
      <Icon name="package" size={17} /> {t("engines.installed")}
      <span class="badge muted">{installed.length}</span>
    </div>
    {#if installed.length}
      <div class="grid-2">
        {#each installed as c (c.id)}
          {@const active = store.engineRunning}
          <div class="list-item">
            <span class="dot" class:ok={active}></span>
            <div class="grow" style="min-width:0">
              <div class="name">{c.name}</div>
              <div class="sub">
                {c.version ? `v${c.version.replace(/^v/i, "")}` : t("common.installed")} · {t("home.dpiEngine")}
              </div>
            </div>
            <button
              class="btn small"
              class:primary={!active}
              class:danger={active}
              onclick={() => store.togglePower()}
              disabled={store.busy || !store.elevated}
            >
              {active ? t("common.stop") : t("common.start")}
            </button>
          </div>
        {/each}
      </div>
      <div class="small dim" style="margin-top:10px;line-height:1.5">
        {t("home.singleEngineNote")}
      </div>
    {:else}
      <div class="empty">
        {t("home.noEngine")}
      </div>
    {/if}
  </div>

  {#if store.progress.zapret && !store.progress.zapret.stage.includes("Готово")}
    <div class="card flat">
      <div class="row-between small">
        <span>{store.progress.zapret.stage}</span>
        <span class="dim">{store.progress.zapret.percent}%</span>
      </div>
      <div class="progress" style="margin-top:9px"><span style="width:{store.progress.zapret.percent}%"></span></div>
    </div>
  {/if}

  {#if store.serviceResults && overall}
    <div class="card">
      <div class="card-title">
        <Icon name="bolt" size={17} /> {t("home.checkResult")}
        <span class="badge" class:ok={overall.pc >= 90} class:warn={overall.pc < 90 && overall.pc > 0} class:bad={overall.pc === 0}>
          {overall.pc}% · {overall.ok}/{overall.total}
        </span>
      </div>
      <div class="grid-3">
        {#each groups as g (g.id)}
          <div class="list-item">
            <span class="dot" class:ok={g.pc === 100} class:warn={g.pc > 0 && g.pc < 100} class:bad={g.pc === 0}></span>
            <div class="grow">
              <div class="name">{g.title}</div>
              <div class="sub">{t("home.requestsOf", { ok: g.ok, total: g.total })}</div>
            </div>
            <span class="badge" class:ok={g.pc === 100} class:warn={g.pc > 0 && g.pc < 100} class:bad={g.pc === 0}>{g.pc}%</span>
          </div>
        {/each}
      </div>
      {#if groups.some((g) => g.pc < 100)}
        <details style="margin-top:12px">
          <summary class="small muted" style="cursor:pointer">{t("home.whatFailed")}</summary>
          <div class="list" style="margin-top:9px">
            {#each groups.filter((g) => g.pc < 100) as g (g.id)}
              {#each g.failed as r (r.url)}
                <div class="small">
                  <span class="badge bad">{r.status ?? "ERR"}</span>
                  <span class="mono wrap">{r.url}</span>
                  {#if r.error}<span class="dim"> — {r.error}</span>{/if}
                </div>
              {/each}
            {/each}
          </div>
        </details>
      {/if}
    </div>
  {/if}

  <div class="grid-2">
    <div class="card">
      <div class="card-title">
        <Icon name="send" size={17} /> {t("engines.tgProxy")}
        <span class="badge" class:ok={proxyOn} class:muted={!proxyOn}>{proxyOn ? t("common.running") : t("home.proxyOff")}</span>
      </div>
      <p class="small muted" style="line-height:1.55">
        {t("home.proxyDesc", { port: settings?.tgws_port ?? "" })}
      </p>
      <div class="row" style="margin-top:14px">
        <button class="btn" class:primary={!proxyOn} class:danger={proxyOn} onclick={() => store.toggleProxy()} disabled={store.busy || !tgws?.installed}>
          <Icon name={proxyOn ? "stop" : "play"} size={15} /> {proxyOn ? t("common.stop") : t("common.start")}
        </button>
        <button class="btn small ghost" onclick={copyLink}><Icon name="copy" size={14} /> {t("home.link")}</button>
        <button class="btn small ghost" onclick={openLink}><Icon name="external" size={14} /> {t("common.open")}</button>
      </div>
      {#if !tgws?.installed}
        <div class="small dim" style="margin-top:10px">{t("home.proxyNotInstalled")}</div>
      {/if}
    </div>

    <div class="card">
      <div class="card-title">
        <Icon name="layers" size={17} /> {t("home.currentProfile")}
        <span class="badge muted">{settings?.engine === "zapret2" ? "Zapret 2" : "Zapret legacy"}</span>
      </div>
      {#if dpiInstalled}
        <div class="tabs" style="margin-bottom:12px">
          <button class:on={settings?.engine === "zapret2"} onclick={() => store.saveSettings({ engine: "zapret2" })}>
            Zapret 2 · winws2
          </button>
          <button class:on={settings?.engine === "zapret"} onclick={() => store.saveSettings({ engine: "zapret" })}>
            Zapret legacy · winws
          </button>
        </div>
      {/if}
      {#if active}
        <div class="row" style="align-items:flex-start">
          <div class="grow">
            <div class="name" style="font-weight:600">{active.name}</div>
            <div class="sub dim small" style="margin-top:4px">
              {active.description ?? (active.tags.length ? active.tags.map((tag) => "#" + tag).join(" ") : t("home.strategyReady"))}
            </div>
          </div>
          {#if active.score != null}<span class="badge ok">{active.score}%</span>{/if}
        </div>
        <div class="row small muted" style="margin-top:12px">
          <span class="badge">{sourceTitle(active.source)}</span>
          {#if active.tcp_ports}<span>TCP {active.tcp_ports}</span>{/if}
          {#if active.game_filter}<span class="badge warn">game filter</span>{/if}
        </div>
      {:else}
        <div class="small muted">{t("home.noStrategy")}</div>
      {/if}
      <div style="margin-top:14px">
        <div class="small dim" style="margin-bottom:6px">{t("home.bypassScope")}</div>
        <div class="tabs">
          <button
            class:on={(settings?.bypass_scope ?? "list") === "list"}
            onclick={() => store.saveSettings({ bypass_scope: "list" })}
            title={t("home.scopeListHint")}
          >
            {t("home.scopeList")}
          </button>
          <button
            class:on={settings?.bypass_scope === "all"}
            onclick={() => store.saveSettings({ bypass_scope: "all" })}
            title={t("home.scopeAllHint")}
          >
            {t("home.scopeAll")}
          </button>
        </div>
        {#if settings?.bypass_scope === "all"}
          <div class="small dim" style="margin-top:8px">
            {t("home.scopeAllNote")}
          </div>
        {/if}
      </div>
      <div class="row" style="margin-top:14px">
        <button class="btn small" onclick={() => (store.screen = "strategies")}>{t("home.changeStrategy")}</button>
        <button class="btn small ghost" onclick={() => (store.screen = "logs")}>{t("nav.logs")}</button>
      </div>
      {#if settings?.last_check}
        <div class="small dim" style="margin-top:10px">{t("home.lastDiagnostics", { time: fmtTime(settings.last_check) })}</div>
      {/if}
    </div>
  </div>
</div>
