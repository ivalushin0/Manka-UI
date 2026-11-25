<script lang="ts">
  import { onMount, tick } from "svelte";
  import Icon from "../lib/Icon.svelte";
  import Modal from "../lib/Modal.svelte";
  import { store } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { copyText, fmtBytes } from "../lib/utils";
  import { locale, t } from "../lib/i18n.svelte";
  import type { LogInfo } from "../lib/types";

  type LogName = "engine" | "tgws" | "app";
  type Level = "error" | "warn" | "info" | "other";

  type Entry = {
    i: number;
    ts: string;
    text: string;
    level: Level;
  };

  const cache = new Map<LogName, { lines: string[]; info: LogInfo | null }>();

  const tabs = $derived<{ id: LogName; title: string; hint: string }[]>([
    { id: "app", title: t("logs.tabApp"), hint: t("logs.hintApp") },
    { id: "engine", title: t("logs.tabEngine"), hint: "winws / winws2" },
    { id: "tgws", title: "Telegram", hint: "tg-ws-proxy" },
  ]);

  let name = $state<LogName>("app");
  let lines = $state<string[]>([]);
  let info = $state<LogInfo | null>(null);
  let auto = $state(true);
  let follow = $state(true);
  let wrap = $state(true);
  let query = $state("");
  let level = $state<"all" | Level>("all");
  let limit = $state(400);
  let expanded = $state<Set<number>>(new Set());
  let loading = $state(false);
  let confirmClear = $state(false);
  let logEl: HTMLDivElement | undefined = $state();

  const LEVEL_RE: Record<Exclude<Level, "other">, RegExp> = {
    error: /\b(error|fatal|panic|failed|failure|refused|denied)\b|ошиб|не удалось|сбой/i,
    warn: /\b(warn|warning|deprecated|retry|timeout)\b|вниман|таймаут/i,
    info: /\b(info|started|stopped|installed|connected|ready|done)\b|запущ|останов|установлен/i,
  };

  function classify(text: string): Level {
    if (LEVEL_RE.error.test(text)) return "error";
    if (LEVEL_RE.warn.test(text)) return "warn";
    if (LEVEL_RE.info.test(text)) return "info";
    return "other";
  }

  function parseLine(line: string, i: number): Entry {
    const m = line.match(/^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\s+([\s\S]*)$/);
    const ts = m ? m[1].slice(11) : "";
    const text = m ? m[2] : line;
    return { i, ts, text, level: classify(line) };
  }

  const entries = $derived(lines.map(parseLine));

  const counts = $derived.by(() => {
    const c = { error: 0, warn: 0, info: 0, other: 0 };
    for (const e of entries) c[e.level]++;
    return c;
  });

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return entries.filter((e) => {
      if (level !== "all" && e.level !== level) return false;
      if (!q) return true;
      return e.text.toLowerCase().includes(q) || e.ts.includes(q);
    });
  });

  const followMode = $derived(auto && follow);

  async function scrollDown(force = false) {
    await tick();
    if (logEl && (force || followMode)) logEl.scrollTop = logEl.scrollHeight;
  }

  async function fetchLines(withScroll = false) {
    const meta = await api.logInfo(name);
    info = meta;
    if (!meta.exists) {
      lines = [];
      cache.set(name, { lines, info: meta });
      return;
    }
    const data = await api.readLog(name, limit);
    lines = data;
    cache.set(name, { lines: data, info: meta });
    if (withScroll) await scrollDown(true);
  }

  async function load(showSpinner = false) {
    if (showSpinner) loading = true;
    try {
      await fetchLines(true);
    } catch (e) {
      store.fail(e);
    } finally {
      loading = false;
    }
  }

  async function poll() {
    if (!auto) return;
    try {
      const meta = await api.logInfo(name);
      const cached = cache.get(name);
      const unchanged =
        cached?.info &&
        cached.info.exists === meta.exists &&
        cached.info.size === meta.size &&
        cached.info.modified === meta.modified;
      info = meta;
      if (unchanged) return;
      await fetchLines(true);
    } catch {
      /* transient while the file is being written */
    }
  }

  async function selectTab(id: LogName) {
    name = id;
    const cached = cache.get(id);
    if (cached) {
      lines = cached.lines;
      info = cached.info;
      limit = Math.max(limit, cached.lines.length);
    } else {
      lines = [];
      limit = 400;
      await load(true);
    }
    await scrollDown(true);
  }

  async function showMore() {
    limit = Math.min(2000, limit + 600);
    await load(true);
  }

  async function clearLog() {
    confirmClear = false;
    try {
      await api.clearLog(name);
      cache.delete(name);
      lines = [];
      info = await api.logInfo(name);
      store.notify(t("logs.cleared"));
    } catch (e) {
      store.fail(e);
    }
  }

  function onScroll() {
    if (!logEl) return;
    const gap = logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight;
    if (gap < 30) follow = true;
    else if (auto) follow = false;
  }

  function toggleExpand(i: number) {
    const next = new Set(expanded);
    if (next.has(i)) next.delete(i);
    else next.add(i);
    expanded = next;
  }

  $effect(() => {
    name;
    auto;
    const timer = window.setInterval(poll, auto ? 2500 : 3_600_000);
    return () => window.clearInterval(timer);
  });

  onMount(() => {
    void selectTab(name);
  });

  $effect(() => {
    if (limit > 400 && followMode) void scrollDown(true);
  });

  function filterActive(): boolean {
    return !!(query.trim() || level !== "all");
  }

  async function copyVisible() {
    const text = filtered.map((e) => (e.ts ? `${e.ts} ${e.text}` : e.text)).join("\n");
    if (await copyText(text)) store.notify(t("logs.copied"));
    else store.fail(t("logs.copyFailed"));
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <div class="page-title">{t("nav.logs")}</div>
      <div class="page-sub">{t("logs.subtitle")}</div>
    </div>
    <div class="row">
      <button class="btn small" onclick={() => load(true)} disabled={loading}>
        <Icon name="refresh" size={14} /> {loading ? t("logs.reading") : t("common.refresh")}
      </button>
      <button class="btn small ghost" onclick={() => api.openLogsDir()}>
        <Icon name="external" size={14} /> {t("logs.folder")}
      </button>
    </div>
  </div>

  <div class="card flat">
    <div class="log-toolbar">
      <div class="tabs">
        {#each tabs as t (t.id)}
          <button
            class:on={name === t.id}
            title={t.hint}
            onclick={() => selectTab(t.id)}
          >
            {t.title}
          </button>
        {/each}
      </div>

      <div class="search grow">
        <Icon name="search" size={15} />
        <input type="text" placeholder={t("logs.searchPh")} bind:value={query} />
      </div>

      <div class="row nowrap">
        <span class="badge muted mono">{fmtBytes(info?.size ?? 0)}</span>
        <span class="badge muted">{t("logs.lines", { count: filtered.length })}</span>
      </div>
    </div>

    <div class="log-controls">
      <div class="row">
        <button class="chip" class:on={level === "all"} onclick={() => (level = "all")}>
          {t("common.all")} <span class="dim">{counts.error + counts.warn + counts.info + counts.other}</span>
        </button>
        <button class="chip" class:on={level === "error"} onclick={() => (level = "error")}>
          <span class="dot bad"></span> {t("logs.errors")} <span class="dim">{counts.error}</span>
        </button>
        <button class="chip" class:on={level === "warn"} onclick={() => (level = "warn")}>
          <span class="dot warn"></span> {t("logs.warnings")} <span class="dim">{counts.warn}</span>
        </button>
        <button class="chip" class:on={level === "info"} onclick={() => (level = "info")}>
          <span class="dot ok"></span> {t("logs.events")} <span class="dim">{counts.info}</span>
        </button>
      </div>

      <div class="row">
        <label class="check small"><input type="checkbox" bind:checked={auto} /> {t("logs.follow")}</label>
        <label class="check small"><input type="checkbox" bind:checked={wrap} /> {t("logs.wrap")}</label>
        <button class="btn small ghost" onclick={() => copyVisible()}>
          <Icon name="copy" size={14} /> {t("common.copy")}
        </button>
        <button class="btn small danger" onclick={() => (confirmClear = true)} title={t("logs.clearTitle")}>
          <Icon name="trash" size={14} />
        </button>
      </div>
    </div>

    <div
      class="logview"
      class:nowrap={!wrap}
      bind:this={logEl}
      onscroll={onScroll}
      role="log"
      aria-live="polite"
    >
      {#if filtered.length === 0}
        <div class="log-empty">
          {#if lines.length === 0}
            {t("logs.empty")}
          {:else}
            {t("logs.noMatch")}
          {/if}
        </div>
      {:else}
        {#each filtered as e (e.i)}
          <div class="logline lvl-{e.level}" class:cont={!e.ts}>
            <span class="ln">{e.i + 1}</span>
            {#if e.ts}<span class="ts">{e.ts}</span>{:else}<span class="ts">·</span>{/if}
            <span class="msg">
              {#if e.text.length > 320 && !expanded.has(e.i)}
                {e.text.slice(0, 320)}<button class="more" onclick={() => toggleExpand(e.i)}>{t("logs.showFull")}</button>
              {:else}
                {e.text}
                {#if e.text.length > 320}
                  <button class="more" onclick={() => toggleExpand(e.i)}>{t("logs.collapse")}</button>
                {/if}
              {/if}
            </span>
          </div>
        {/each}
      {/if}
    </div>

    <div class="log-foot">
      <div class="row small dim">
        {#if info?.modified}
          <span>{t("logs.updated", { time: new Date(info.modified).toLocaleTimeString(locale()) })}</span>
        {/if}
        <span>{t("logs.loaded", { count: lines.length })}</span>
        {#if filterActive()}<span>{t("logs.filtered", { count: filtered.length })}</span>{/if}
      </div>
      <div class="row">
        {#if lines.length >= limit && limit < 2000}
          <button class="btn small ghost" onclick={showMore}>{t("logs.showMore")}</button>
        {/if}
        {#if followMode}
          <span class="badge ok"><span class="dot ok"></span> {t("logs.autoscroll")}</span>
        {:else if auto}
          <button class="btn small ghost" onclick={() => { follow = true; scrollDown(true); }}>
            {t("logs.down")}
          </button>
        {/if}
      </div>
    </div>
  </div>
</div>

{#if confirmClear}
  <Modal
    title={t("logs.clearModalTitle")}
    subtitle={t("logs.clearModalText")}
    onclose={() => (confirmClear = false)}
  >
    <div class="row" style="justify-content:flex-end">
      <button class="btn" onclick={() => (confirmClear = false)}>{t("common.cancel")}</button>
      <button class="btn danger" onclick={clearLog}>
        <Icon name="trash" size={14} /> {t("logs.clear")}
      </button>
    </div>
  </Modal>
{/if}

<style>
  .log-toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  .log-controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
    margin: 12px 0;
  }

  .nowrap {
    flex-wrap: nowrap;
  }

  .logview {
    background: #060a12;
    border: 1px solid var(--line-soft);
    border-radius: 12px;
    height: min(52vh, 560px);
    overflow: auto;
    font-family: "Cascadia Mono", Consolas, monospace;
    font-size: 11.5px;
    line-height: 1.55;
    padding: 6px 0;
  }

  .logline {
    display: flex;
    gap: 10px;
    padding: 1px 12px;
    color: #aab7cf;
    border-left: 2px solid transparent;
    content-visibility: auto;
    contain-intrinsic-size: auto 18px;
  }

  .logline:hover {
    background: rgba(124, 92, 255, 0.06);
  }

  .logline.lvl-error {
    color: #ffc2c2;
    background: rgba(239, 68, 68, 0.07);
    border-left-color: #ef4444;
  }
  .logline.lvl-warn {
    color: #fcd58b;
    background: rgba(245, 158, 11, 0.055);
    border-left-color: #f59e0b;
  }
  .logline.lvl-info {
    color: #b7c3da;
  }
  .logline.lvl-info .ts {
    color: #64748b;
  }

  .ln {
    color: #35425a;
    min-width: 42px;
    text-align: right;
    user-select: none;
    flex: none;
  }

  .ts {
    color: #5d6b84;
    flex: none;
    user-select: none;
  }

  .msg {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    min-width: 0;
  }

  .logview.nowrap .msg {
    white-space: pre;
    overflow-wrap: normal;
  }

  .more {
    margin-left: 6px;
    border: 0;
    background: transparent;
    color: var(--accent-2);
    font: inherit;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .log-empty {
    padding: 28px;
    text-align: center;
    color: var(--dim);
    font-family: "Segoe UI", system-ui, sans-serif;
    font-size: 13px;
  }

  .log-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 10px;
    flex-wrap: wrap;
  }
</style>
