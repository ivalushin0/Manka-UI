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
  import type { AutoState, Progress } from "./lib/types";
  import { api } from "./lib/api";

  const nav: { id: Screen; label: string; icon: string }[] = [
    { id: "home", label: "Главная", icon: "home" },
    { id: "strategies", label: "Стратегии", icon: "layers" },
    { id: "auto", label: "Автоподбор", icon: "wand" },
    { id: "components", label: "Компоненты", icon: "package" },
    { id: "telegram", label: "Telegram", icon: "send" },
    { id: "logs", label: "Журнал", icon: "terminal" },
    { id: "settings", label: "Настройки", icon: "sliders" },
  ];

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
    store.init().then(() => {
      const s = store.settings;
      if (!s) return;
      const interval = Math.max(1, s.update_interval_hours || 6) * 3600_000;
      if (Date.now() - (s.last_update_check || 0) > interval) {
        store.checkUpdates();
      }
      if (s.update_repo?.trim() && s.auto_update_check) {
        const appInterval = Math.max(1, s.app_update_interval_hours || 24) * 3600_000;
        if (Date.now() - (s.last_app_update_check || 0) > appInterval) {
          void store.checkAppUpdate(true);
        }
      }
    });
    const statusTimer = window.setInterval(() => store.refreshStatus(), 3000);
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
      );
    })();
    return () => {
      window.clearInterval(statusTimer);
      offs.forEach((off) => off());
    };
  });
</script>

{#if store.loading}
  <div class="boot">
    <div class="brand-logo" style="width:64px;height:64px;font-size:32px;border-radius:20px">M</div>
    <div>
      <div style="font-weight:650;font-size:18px">Manka</div>
      <div class="dim small">Загружаю настройки…</div>
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
            {#if item.id === "settings" && store.appUpdate}
              <span class="nav-dot" title={`Доступно обновление ${store.appUpdate.version}`}></span>
            {/if}
          </button>
        {/each}
      </nav>

      <div class="sidebar-foot">
        <div class="sidebar-status">
          <span class="dot" class:ok={store.engineRunning} class:warn={!store.engineRunning && store.settings?.enabled}></span>
          <span class="text">{store.engineRunning ? "Обход работает" : "Обход выключен"}</span>
        </div>
        <div class="sidebar-status">
          <span class="dot" class:ok={store.data?.proxy.running}></span>
          <span class="text">{store.data?.proxy.running ? `Прокси :${store.settings?.tgws_port}` : "Прокси выключен"}</span>
        </div>
        <div class="text">v{store.data?.version} · {store.elevated ? "администратор" : "обычный режим"}</div>
      </div>
    </aside>

    <main class="main">
      {#if !store.elevated}
        <div class="banner" style="max-width:1000px;margin:0 auto 14px">
          <Icon name="shield" size={20} />
          <span class="grow">
            Manka запущена без прав администратора: WinDivert не загрузится, и обход не сможет работать.
          </span>
          <button class="btn small primary" onclick={relaunch} disabled={relaunching}>
            {relaunching ? "Перезапуск…" : "Перезапустить от администратора"}
          </button>
        </div>
      {/if}

      {#if store.appUpdate}
        <div class="banner update" style="max-width:1000px;margin:0 auto 14px">
          <Icon name="download" size={20} />
          <span class="grow">
            Доступна новая версия Manka <b>{store.appUpdate.version}</b>
            {#if store.appUpdate.date} от {new Date(store.appUpdate.date).toLocaleDateString("ru-RU")}{/if}.
          </span>
          <button class="btn small ghost" onclick={() => (store.screen = "settings")}>
            Что нового
          </button>
          <button class="btn small primary" onclick={() => store.installAppUpdate()} disabled={store.appUpdateInstalling}>
            {store.appUpdateInstalling ? "Обновляю…" : "Обновить"}
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
