<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { store, type Screen } from "./lib/store.svelte";
  import Icon from "./lib/Icon.svelte";
  import Home from "./screens/Home.svelte";
  import Strategies from "./screens/Strategies.svelte";
  import Auto from "./screens/Auto.svelte";
  import Components from "./screens/Components.svelte";
  import Telegram from "./screens/Telegram.svelte";
  import SettingsScreen from "./screens/Settings.svelte";
  import Logs from "./screens/Logs.svelte";
  import Doctor from "./screens/Doctor.svelte";
  import type { AutoState, Progress } from "./lib/types";
  import { api } from "./lib/api";
  import { locale, t } from "./lib/i18n.svelte";

  const nav = $derived<{ id: Screen; label: string; icon: string }[]>([
    { id: "home", label: t("nav.home"), icon: "home" },
    { id: "strategies", label: t("nav.strategies"), icon: "layers" },
    { id: "auto", label: t("nav.auto"), icon: "wand" },
    { id: "components", label: t("nav.components"), icon: "package" },
    { id: "telegram", label: t("nav.telegram"), icon: "send" },
    { id: "doctor", label: t("nav.doctor"), icon: "stethoscope" },
    { id: "logs", label: t("nav.logs"), icon: "terminal" },
    { id: "settings", label: t("nav.settings"), icon: "sliders" },
  ]);

  let relaunching = $state(false);

  async function relaunch() {
    relaunching = true;
    try {
      await api.relaunchElevated();
    } catch (e) {
      store.fail(e);
      relaunching = false;
    }
  }

  onMount(() => {
    const offs: (() => void)[] = [];
    let lastFocusCheck = 0;

    function autoCheckConfigured() {
      const s = store.settings;
      if (!s?.update_repo?.trim() || !s.auto_update_check) return;
      const interval = Math.max(1, s.app_update_interval_hours || 24) * 3600_000;
      if (Date.now() - (s.last_app_update_check || 0) > interval) void store.checkAppUpdate(true);
    }

    function autoCheckNow() {
      const s = store.settings;
      if (!s?.update_repo?.trim() || !s.auto_update_check) return;
      lastFocusCheck = Date.now();
      void store.checkAppUpdate(true);
    }

    store.init().then(() => {
      const s = store.settings;
      if (!s) return;
      const interval = Math.max(1, s.update_interval_hours || 6) * 3600_000;
      if (Date.now() - (s.last_update_check || 0) > interval) {
        store.checkUpdates();
      }
      // Always look for a new app version at launch, regardless of the saved
      // interval: otherwise a release lands silently and the user never sees
      // the toast or the update card.
      autoCheckNow();
    });

    const statusTimer = window.setInterval(() => store.refreshStatus(), 3000);
    const updateTimer = window.setInterval(autoCheckConfigured, 15 * 60_000);
    const onFocus = () => {
      if (Date.now() - lastFocusCheck < 30 * 60_000) return;
      autoCheckNow();
    };
    window.addEventListener("focus", onFocus);
    (async () => {
      offs.push(
        await listen<Progress>("component-progress", (e) => store.onProgress(e.payload)),
        await listen<Progress>("store-progress", (e) => store.onProgress(e.payload)),
        await listen<AutoState>("auto-progress", (e) => store.onAuto(e.payload)),
        await listen<{ downloaded: number; total: number | null }>("app-update-progress", (e) =>
          store.onAppUpdateProgress(e.payload),
        ),
        await listen("components-changed", () => store.refresh()),
        await listen("store-changed", () => store.refresh()),
        await listen("status-changed", () => store.refresh()),
        await listen<import("./lib/types").WatchdogReport>("watchdog", (e) => store.onWatchdog(e.payload)),
        await listen<{ name: string; percent: number }>("watchdog-fixed", (e) => store.onWatchdogFixed(e.payload)),
        await listen("watchdog-failed", () => store.onWatchdogFailed()),
        await listen<import("./lib/types").ListsReport>("lists-updated", (e) => store.onListsUpdated(e.payload)),
      );
    })();
    return () => {
      window.clearInterval(statusTimer);
      window.clearInterval(updateTimer);
      window.removeEventListener("focus", onFocus);
      offs.forEach((off) => off());
    };
  });
</script>

{#if store.loading}
  <div class="boot">
    <div class="brand-logo" style="width:64px;height:64px;font-size:32px;border-radius:20px">M</div>
    <div>
      <div style="font-weight:650;font-size:18px">Manka</div>
      <div class="dim small">{t("app.loading")}</div>
    </div>
  </div>
{:else}
  <div class="app">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-logo">M</div>
        <div class="brand-text">
          <div class="brand-name">Manka</div>
          <div class="brand-sub">DPIslayer for Windows</div>
        </div>
      </div>

      <nav class="nav">
        {#each nav as item (item.id)}
          <button class:active={store.screen === item.id} onclick={() => (store.screen = item.id)}>
            <span class="icon"><Icon name={item.icon} /></span>
            <span>{item.label}</span>
            {#if (item.id === "settings" || item.id === "home") && store.appUpdate}
              <span class="nav-dot" title={t("app.updateAvailable", { version: store.appUpdate.version })}></span>
            {/if}
          </button>
        {/each}
      </nav>

      <div class="sidebar-foot">
        <div class="sidebar-status">
          <span class="dot" class:ok={store.engineRunning} class:warn={!store.engineRunning && store.settings?.enabled}></span>
          <span class="text">{store.engineRunning ? t("app.bypassOn") : t("app.bypassOff")}</span>
        </div>
        <div class="sidebar-status">
          <span class="dot" class:ok={store.data?.proxy.running}></span>
          <span class="text">
            {store.data?.proxy.running ? t("app.proxyPort", { port: store.settings?.tgws_port ?? "" }) : t("app.proxyOff")}
          </span>
        </div>
        <div class="text">v{store.data?.version} · {store.elevated ? t("app.admin") : t("app.user")}</div>
      </div>
    </aside>

    <main class="main">
      {#if !store.elevated}
        <div class="banner" style="max-width:1000px;margin:0 auto 14px">
          <Icon name="shield" size={20} />
          <span class="grow">{t("app.noAdmin")}</span>
          <button class="btn small primary" onclick={relaunch} disabled={relaunching}>
            {relaunching ? t("app.relaunching") : t("app.relaunchAdmin")}
          </button>
        </div>
      {/if}

      {#if store.appUpdate && store.screen !== "home"}
        <div class="banner update" style="max-width:1000px;margin:0 auto 14px">
          <Icon name="download" size={20} />
          <span class="grow">
            {t("app.updateBanner")} <b>{store.appUpdate.version}</b>
            {#if store.appUpdate.date}
              {t("app.updateFrom", { date: new Date(store.appUpdate.date).toLocaleDateString(locale()) })}.
            {/if}
          </span>
          <button class="btn small ghost" onclick={() => (store.screen = "settings")}>
            {t("app.whatsNew")}
          </button>
          <button class="btn small primary" onclick={() => store.installAppUpdate()} disabled={store.appUpdateInstalling}>
            {store.appUpdateInstalling ? t("app.updating") : t("app.update")}
          </button>
        </div>
      {/if}

      {#if store.screen === "home"}
        <Home />
      {:else if store.screen === "strategies"}
        <Strategies />
      {:else if store.screen === "auto"}
        <Auto />
      {:else if store.screen === "components"}
        <Components />
      {:else if store.screen === "telegram"}
        <Telegram />
      {:else if store.screen === "doctor"}
        <Doctor />
      {:else if store.screen === "settings"}
        <SettingsScreen />
      {:else if store.screen === "logs"}
        <Logs />
      {/if}
    </main>
  </div>
{/if}

{#if store.error}
  <div class="toast err">{store.error}</div>
{:else if store.notice}
  <div class="toast">{store.notice}</div>
{/if}

<style>
  .boot {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 18px;
  }

  .nav-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 10px rgba(34, 197, 94, 0.85);
    margin-left: auto;
    flex: none;
  }

  .banner.update {
    border-color: rgba(34, 197, 94, 0.38);
    background: rgba(34, 197, 94, 0.08);
    color: #c9f7db;
  }
</style>
