<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../lib/Icon.svelte";
  import { store } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { locale, plural, t } from "../lib/i18n.svelte";

  onMount(() => {
    if (!store.doctorReport) void store.runDoctor();
  });

  const report = $derived(store.doctorReport);

  async function exportAndOpen() {
    const path = await store.exportDoctor();
    if (path) await api.openLogsDir();
  }

  function statusIcon(status: string): string {
    return status === "ok" ? "check" : status === "warn" ? "info" : "x";
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <div class="page-title">{t("nav.doctor")}</div>
      <div class="page-sub">
        {t("doctor.subtitle")}
      </div>
    </div>
    <div class="row">
      <button class="btn" onclick={() => store.runDoctor()} disabled={store.doctorBusy}>
        <Icon name="refresh" size={15} /> {store.doctorBusy ? t("common.checking") : t("doctor.recheck")}
      </button>
      <button class="btn primary" onclick={exportAndOpen} disabled={store.doctorExporting}>
        <Icon name="download" size={15} />
        {store.doctorExporting ? t("doctor.exporting") : t("doctor.export")}
      </button>
    </div>
  </div>

  <div class="card" style="margin-bottom:14px">
    <div class="card-title"><Icon name="shield" size={17} /> {t("doctor.watchdog")}</div>
    <div class="small muted" style="margin-bottom:10px">
      {t("doctor.watchdogLead", { interval: store.settings?.watchdog_interval_min ?? 15 })}
      {store.settings?.watchdog_auto_fix ? t("doctor.watchdogAutoFix") : t("doctor.watchdogNotify")}
      {t("doctor.watchdogTail")}
      {#if store.settings?.last_watchdog_check}
        {t("doctor.lastCheck", { date: new Date(store.settings.last_watchdog_check).toLocaleString(locale()) })}.
      {/if}
    </div>
    <div class="row">
      <span class="badge" class:ok={store.settings?.watchdog_enabled} class:muted={!store.settings?.watchdog_enabled}>
        {store.settings?.watchdog_enabled ? t("doctor.enabled") : t("doctor.disabled")}
      </span>
      <button class="btn small ghost" onclick={() => store.watchdogCheckNow()} disabled={store.watchdogBusy}>
        <Icon name="search" size={14} /> {store.watchdogBusy ? t("common.checking") : t("common.checkNow")}
      </button>
    </div>
  </div>

  {#if report}
    <div class="row" style="margin-bottom:12px;gap:8px">
      <span class="badge ok">{report.ok} {t("doctor.okWord")}</span>
      {#if report.warn}<span class="badge warn">{plural(report.warn, "doctor.warnForms")}</span>{/if}
      {#if report.fail}<span class="badge err">{plural(report.fail, "doctor.failForms")}</span>{/if}
      <span class="dim small" style="margin-left:auto">
        {new Date(report.generated_at).toLocaleString(locale())}
      </span>
    </div>

    <div class="list">
      {#each report.checks as check (check.id)}
        <div class="check-item {check.status}">
          <span class="check-icon {check.status}">
            <Icon name={statusIcon(check.status)} size={16} />
          </span>
          <div class="grow">
            <div class="check-title">{check.title}</div>
            <div class="small muted wrap">{check.detail}</div>
          </div>
        </div>
      {/each}
    </div>

    {#if report.fail > 0 || report.warn > 0}
      <div class="card" style="margin-top:14px">
        <div class="card-title"><Icon name="external" size={16} /> {t("doctor.notHelped")}</div>
        <div class="small muted">
          {t("doctor.exportText")}
        </div>
      </div>
    {/if}
  {:else if store.doctorBusy}
    <div class="card"><div class="dim">{t("doctor.running")}</div></div>
  {/if}
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .check-item {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 14px;
    border: 1px solid var(--line-soft);
    border-radius: 12px;
    background: rgba(15, 21, 34, 0.55);
  }

  .check-icon {
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 9px;
    display: grid;
    place-items: center;
  }

  .check-icon.ok {
    color: var(--ok);
    background: rgba(34, 197, 94, 0.12);
  }

  .check-icon.warn {
    color: var(--warn, #f59e0b);
    background: rgba(245, 158, 11, 0.12);
  }

  .check-icon.fail {
    color: var(--err, #ef4444);
    background: rgba(239, 68, 68, 0.12);
  }

  .check-title {
    font-weight: 600;
    margin-bottom: 2px;
  }

  .wrap {
    word-break: break-word;
  }
</style>
