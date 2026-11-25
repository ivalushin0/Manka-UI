<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import Markdown from "../lib/Markdown.svelte";
  import { store } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { locale, t } from "../lib/i18n.svelte";

  const settings = $derived(store.settings);
  const repo = $derived(settings?.update_repo?.trim() ?? "");
  const updatePc = $derived.by(() => {
    const p = store.appUpdateProgress;
    if (!p || !p.total) return null;
    return Math.min(100, Math.round((p.downloaded / p.total) * 100));
  });

  function toggleGroup(id: string) {
    if (!settings) return;
    const next = settings.targets.includes(id)
      ? settings.targets.filter((x) => x !== id)
      : [...settings.targets, id];
    store.saveSettings({ targets: next });
  }

  async function autostart(enabled: boolean) {
    try {
      await api.setAutostart(enabled);
      await store.saveSettings({ autostart: enabled });
      store.notify(enabled ? t("settings.autostartOn") : t("settings.autostartOff"));
    } catch (e) {
      store.fail(e);
    }
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <div class="page-title">{t("nav.settings")}</div>
      <div class="page-sub">{t("settings.subtitle")}</div>
    </div>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="shield" size={18} /> {t("settings.startup")}</div>
    <div class="grid-2">
      <label class="field">
        {t("settings.engine")}
        <select
          value={settings?.engine}
          onchange={(e) => store.saveSettings({ engine: (e.target as HTMLSelectElement).value as "zapret2" | "zapret" })}
        >
          <option value="zapret2">Zapret 2 (winws2)</option>
          <option value="zapret">Zapret legacy (winws)</option>
        </select>
      </label>
      <label class="field">
        {t("settings.fakeSni")}
        <input
          type="text"
          value={settings?.fake_sni}
          onchange={(e) => store.saveSettings({ fake_sni: (e.target as HTMLInputElement).value })}
        />
      </label>
    </div>

    <div class="list" style="margin-top:16px">
      <label class="check">
        <input
          type="checkbox"
          checked={settings?.autostart ?? false}
          onchange={(e) => autostart((e.target as HTMLInputElement).checked)}
        />
        {t("settings.autostart")}
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={settings?.close_to_tray ?? true}
          onchange={(e) => store.saveSettings({ close_to_tray: (e.target as HTMLInputElement).checked })}
        />
        {t("settings.closeToTray")}
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={settings?.start_minimized ?? true}
          onchange={(e) => store.saveSettings({ start_minimized: (e.target as HTMLInputElement).checked })}
        />
        {t("settings.startMinimized")}
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={settings?.debug_logs ?? false}
          onchange={(e) => store.saveSettings({ debug_logs: (e.target as HTMLInputElement).checked })}
        />
        {t("settings.debugLogs")}
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={settings?.block_quic ?? true}
          onchange={(e) => store.saveSettings({ block_quic: (e.target as HTMLInputElement).checked })}
        />
        {t("settings.blockQuic")}
      </label>
    </div>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="sliders" size={18} /> {t("settings.language")}</div>
    <label class="field">
      {t("settings.languageLabel")}
      <select
        value={settings?.language ?? "ru"}
        onchange={(e) => store.saveSettings({ language: (e.target as HTMLSelectElement).value })}
      >
        <option value="ru">Русский</option>
        <option value="en">English</option>
        <option value="uk">Українська</option>
      </select>
    </label>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="bolt" size={18} /> {t("settings.diagTargets")}</div>
    <div class="row">
      {#each store.data?.target_groups ?? [] as g (g.id)}
        <button class="chip" class:on={settings?.targets.includes(g.id)} onclick={() => toggleGroup(g.id)}>
          {g.title}
        </button>
      {/each}
    </div>
    <label class="field" style="margin-top:14px">
      {t("settings.customTargets")}
      <textarea
        placeholder="example.com"
        value={settings?.custom_targets}
        onchange={(e) => store.saveSettings({ custom_targets: (e.target as HTMLTextAreaElement).value })}
      ></textarea>
    </label>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="stethoscope" size={18} /> {t("doctor.watchdog")}</div>
    <div class="small muted" style="margin-bottom:12px">
      {t("settings.watchdogDesc")}
    </div>
    <div class="list">
      <label class="check">
        <input
          type="checkbox"
          checked={settings?.watchdog_enabled ?? true}
          onchange={(e) => store.saveSettings({ watchdog_enabled: (e.target as HTMLInputElement).checked })}
        />
        {t("settings.watchdogEnabled")}
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={settings?.watchdog_auto_fix ?? true}
          onchange={(e) => store.saveSettings({ watchdog_auto_fix: (e.target as HTMLInputElement).checked })}
        />
        {t("settings.watchdogAutoFix")}
      </label>
    </div>
    <div class="grid-2" style="margin-top:12px">
      <label class="field">
        {t("settings.watchdogInterval")}
        <input
          type="number"
          min="5"
          max="240"
          value={settings?.watchdog_interval_min ?? 15}
          onchange={(e) => store.saveSettings({ watchdog_interval_min: Number((e.target as HTMLInputElement).value) })}
        />
      </label>
      <div class="row" style="align-items:flex-end">
        <button class="btn" onclick={() => store.watchdogCheckNow()} disabled={store.watchdogBusy}>
          <Icon name="search" size={15} /> {store.watchdogBusy ? t("common.checking") : t("common.checkNow")}
        </button>
      </div>
    </div>
    {#if settings?.last_watchdog_check}
      <div class="small dim" style="margin-top:10px">
        {t("doctor.lastCheck", { date: new Date(settings.last_watchdog_check).toLocaleString(locale()) })}
      </div>
    {/if}
  </div>

  <div class="card">
    <div class="card-title"><Icon name="layers" size={18} /> {t("settings.lists")}</div>
    <div class="small muted" style="margin-bottom:12px">
      {t("settings.listsDesc")}
    </div>
    <div class="list">
      <label class="check">
        <input
          type="checkbox"
          checked={settings?.lists_auto_update ?? false}
          onchange={(e) => store.saveSettings({ lists_auto_update: (e.target as HTMLInputElement).checked })}
        />
        {t("settings.listsAuto")}
      </label>
    </div>
    <div class="grid-2" style="margin-top:12px">
      <label class="field">
        {t("settings.listsInterval")}
        <input
          type="number"
          min="1"
          max="720"
          value={settings?.lists_update_interval_hours ?? 24}
          onchange={(e) => store.saveSettings({ lists_update_interval_hours: Number((e.target as HTMLInputElement).value) })}
        />
      </label>
      <div class="row" style="align-items:flex-end">
        <button class="btn" onclick={() => store.updateLists()} disabled={store.listsBusy}>
          <Icon name="refresh" size={15} /> {store.listsBusy ? t("components.refreshing") : t("settings.updateNow")}
        </button>
      </div>
    </div>
    <label class="field" style="margin-top:12px">
      {t("settings.listsSources")}
      <textarea
        placeholder="https://example.com/my-list.txt"
        value={settings?.lists_sources}
        onchange={(e) => store.saveSettings({ lists_sources: (e.target as HTMLTextAreaElement).value })}
      ></textarea>
    </label>
    {#if settings?.last_lists_update}
      <div class="small dim" style="margin-top:10px">
        {t("settings.lastListsUpdate", { date: new Date(settings.last_lists_update).toLocaleString(locale()) })}
      </div>
    {/if}
  </div>

  <div class="card">
    <div class="card-title"><Icon name="refresh" size={18} /> {t("settings.compUpdates")}</div>
    <div class="grid-2">
      <label class="field">
        {t("settings.compInterval")}
        <input
          type="number"
          min="1"
          max="168"
          value={settings?.update_interval_hours}
          onchange={(e) => store.saveSettings({ update_interval_hours: Number((e.target as HTMLInputElement).value) })}
        />
      </label>
      <div class="row" style="align-items:flex-end">
        <button class="btn" onclick={() => store.checkUpdates()} disabled={store.busy}>
          <Icon name="refresh" size={15} /> {t("common.checkNow")}
        </button>
      </div>
    </div>
  </div>

  <div class="card">
    <div class="card-title">
      <Icon name="download" size={18} /> {t("settings.appUpdates")}
      <span class="badge muted">v{store.data?.version}</span>
      {#if store.appUpdate}
        <span class="badge ok" style="margin-left:auto">{t("settings.updateAvailable", { version: store.appUpdate.version })}</span>
      {/if}
    </div>

    {#if store.appUpdate}
      <div class="update-box">
        <div class="row" style="margin-bottom:8px">
          <span class="badge ok">{t("settings.newVersion", { version: store.appUpdate.version })}</span>
          {#if store.appUpdate.date}
            <span class="small dim">{new Date(store.appUpdate.date).toLocaleDateString(locale())}</span>
          {/if}
        </div>
        {#if store.appUpdate.notes}
          <div class="notes"><Markdown text={store.appUpdate.notes} /></div>
        {/if}
        <div class="row" style="margin-top:12px">
          <button class="btn primary" onclick={() => store.installAppUpdate()} disabled={store.appUpdateInstalling}>
            <Icon name="download" size={15} />
            {store.appUpdateInstalling ? t("settings.downloading") : t("settings.installRestart")}
          </button>
          <button class="btn ghost small" onclick={() => api.openUrl(store.appUpdate!.releases_url)}>
            <Icon name="external" size={14} /> {t("settings.releasePage")}
          </button>
        </div>
        {#if store.appUpdateInstalling}
          <div style="margin-top:12px">
            <div class="row-between small" style="margin-bottom:6px">
              <span>{t("settings.downloadingUpdate")}</span>
              <span class="dim">{updatePc != null ? `${updatePc}%` : ""}</span>
            </div>
            <div class="progress"><span style="width:{updatePc ?? 12}%"></span></div>
          </div>
        {/if}
      </div>
    {/if}

    <div class="grid-2">
      <label class="field">
        {t("settings.repo")}
        <input
          type="text"
          placeholder={t("settings.repoPh")}
          value={repo}
          onchange={(e) => store.saveSettings({ update_repo: (e.target as HTMLInputElement).value.trim() })}
        />
      </label>
      <label class="field">
        {t("settings.appInterval")}
        <input
          type="number"
          min="1"
          max="720"
          value={settings?.app_update_interval_hours ?? 24}
          onchange={(e) => store.saveSettings({ app_update_interval_hours: Number((e.target as HTMLInputElement).value) })}
        />
      </label>
    </div>

    <label class="check" style="margin-top:14px">
      <input
        type="checkbox"
        checked={settings?.auto_update_check ?? true}
        onchange={(e) => store.saveSettings({ auto_update_check: (e.target as HTMLInputElement).checked })}
      />
      {t("settings.autoCheck")}
    </label>

    <div class="row" style="margin-top:14px">
      <button class="btn" onclick={() => store.checkAppUpdate()} disabled={store.appUpdateChecking || !repo}>
        <Icon name="refresh" size={15} />
        {store.appUpdateChecking ? t("common.checking") : t("settings.checkApp")}
      </button>
      <button
        class="btn ghost small"
        onclick={() => api.openUrl(repo ? `https://github.com/${repo}/releases` : "https://github.com")}
      >
        <Icon name="external" size={14} /> {t("settings.releases")}
      </button>
    </div>

    <div class="small dim" style="margin-top:12px;line-height:1.55">
      {t("settings.updateNote")}
    </div>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="file" size={18} /> {t("settings.data")}</div>
    <div class="kv">
      <span class="k">{t("settings.dataDir")}</span>
      <span class="row">
        <span class="mono wrap grow">{store.data?.data_dir}</span>
        <button class="btn small ghost" onclick={() => api.openDataDir()}><Icon name="external" size={14} /> {t("common.open")}</button>
      </span>
      <span class="k">{t("settings.rights")}</span>
      <span>
        {#if store.elevated}
          <span class="badge ok">{t("app.admin")}</span>
        {:else}
          <span class="badge warn">{t("settings.regularUser")}</span>
          <button class="btn small" style="margin-left:8px" onclick={() => api.relaunchElevated()}>
            {t("app.relaunchAdmin")}
          </button>
        {/if}
      </span>
      <span class="k">{t("settings.version")}</span><span class="mono">{store.data?.version}</span>
    </div>
    <div class="small dim" style="margin-top:12px;line-height:1.55">
      {t("settings.dataNote")}
    </div>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="info" size={18} /> {t("settings.about")}</div>
    <div class="small muted" style="line-height:1.6">
      {t("settings.aboutText")}
    </div>
  </div>
</div>

<style>
  .update-box {
    border: 1px solid rgba(34, 197, 94, 0.34);
    background: rgba(34, 197, 94, 0.06);
    border-radius: 12px;
    padding: 14px;
    margin-bottom: 16px;
  }

  .notes {
    max-height: 190px;
    overflow: auto;
    padding: 10px 12px;
    border-radius: 9px;
    background: rgba(6, 10, 18, 0.55);
    border: 1px solid var(--line-soft);
  }
</style>
