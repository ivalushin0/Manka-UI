<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import Modal from "../lib/Modal.svelte";
  import { store } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { sourceTitle } from "../lib/utils";
  import { t } from "../lib/i18n.svelte";
  import type { Engine, Strategy } from "../lib/types";

  let engine = $state<Engine>(store.settings?.engine ?? "zapret2");
  let query = $state("");
  let source = $state("all");
  let expanded = $state<string | null>(null);
  let showCatalog = $state(false);
  let shown = $state<Strategy[]>([]);
  let catalogLoading = $state(false);
  let pendingDelete = $state<Strategy | null>(null);

  const activeId = $derived(store.settings?.active[engine]);
  const currentEngine = $derived(store.settings?.engine);

  function isSelected(s: Strategy): boolean {
    return s.id === activeId;
  }

  function isRunning(s: Strategy): boolean {
    return isSelected(s) && currentEngine === s.engine && store.engineRunning;
  }

  const base = $derived.by(() => {
    const list = [...store.strategies, ...(showCatalog ? shown : [])];
    return list.filter((s) => s.engine === engine);
  });

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return base
      .filter((s) => {
        if (source !== "all" && s.source !== source) return false;
        if (!q) return true;
        return (
          s.name.toLowerCase().includes(q) ||
          (s.description ?? "").toLowerCase().includes(q) ||
          s.tags.some((t) => t.toLowerCase().includes(q))
        );
      })
      .sort((a, b) => {
        const ac = isSelected(a) ? 1 : 0;
        const bc = isSelected(b) ? 1 : 0;
        return bc - ac || (b.score ?? -1) - (a.score ?? -1) || a.name.localeCompare(b.name, "ru");
      });
  });

  const counts = $derived.by(() => {
    const map: Record<string, number> = { all: base.length };
    for (const s of base) map[s.source] = (map[s.source] ?? 0) + 1;
    return map;
  });

  async function toggleCatalog() {
    showCatalog = !showCatalog;
    if (showCatalog) {
      catalogLoading = true;
      shown = await api.catalog(engine, false).catch((e) => {
        store.fail(e);
        return [];
      });
      catalogLoading = false;
    }
  }

  async function confirmDelete() {
    const s = pendingDelete;
    pendingDelete = null;
    if (!s) return;
    await store.guard(async () => {
      await api.deleteStrategy(s.id);
      await store.refresh();
      store.notify(t("strategies.deleted", { name: s.name }));
    });
  }

  async function apply(s: Strategy) {
    if (!store.settings) return;
    await store.guard(async () => {
      const saved = await api.setActiveStrategy(s.engine, s.id);
      if (store.data) store.data.settings = saved;
      await store.refresh();
      store.notify(t("strategies.applied", { name: s.name }));
    });
  }

  function optionValue(s: Strategy, variable: string): number {
    return store.settings?.option_value[`${s.id}::${variable}`] ?? -1;
  }

  function flagValue(s: Strategy, key: string, def: boolean): boolean {
    return store.settings?.flag_value[`${s.id}::${key}`] ?? def;
  }

  function badgeClass(s: Strategy): string {
    if (s.source === "store") return "store";
    if (s.source === "auto") return "auto";
    if (s.source === "catalog") return "muted";
    return "";
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <div class="page-title">{t("nav.strategies")}</div>
      <div class="page-sub">{t("strategies.subtitle")}</div>
    </div>
    <div class="tabs">
      <button class:on={engine === "zapret2"} onclick={() => (engine = "zapret2")}>Zapret 2</button>
      <button class:on={engine === "zapret"} onclick={() => (engine = "zapret")}>Zapret legacy</button>
    </div>
  </div>

  <div class="card flat">
    <div class="row">
      <div class="search">
        <Icon name="search" size={15} />
        <input type="text" placeholder={t("strategies.searchPh")} bind:value={query} />
      </div>
      <button class="btn" class:primary={showCatalog} onclick={toggleCatalog} disabled={catalogLoading}>
        <Icon name="layers" size={15} />
        {catalogLoading ? t("common.loading") : showCatalog ? t("strategies.hideCatalog") : t("strategies.showCatalog")}
      </button>
    </div>
    <div class="row" style="margin-top:12px">
      {#each [["all", t("common.all")], ["builtin", t("strategies.filterBuiltin")], ["store", t("strategies.filterStore")], ["auto", t("strategies.filterAuto")], ["user", t("strategies.filterUser")]] as [id, title] (id)}
        <button class="chip" class:on={source === id} onclick={() => (source = id)}>
          {title}
          <span class="dim">{counts[id] ?? 0}</span>
        </button>
      {/each}
    </div>
  </div>

  <div class="small dim" style="margin-top:-6px">
    {t("strategies.note")}
  </div>

  {#if filtered.length === 0}
    <div class="empty">
      {t("strategies.empty")}
    </div>
  {/if}

  <div class="list">
    {#each filtered as s (s.id)}
      <div class="list-item" class:active={isSelected(s)} style="align-items:flex-start">
        <span class="dot" class:ok={isRunning(s)} style="margin-top:6px"></span>
        <div class="grow">
          <div class="row">
            <span class="name">{s.name}</span>
            <span class="badge {badgeClass(s)}">{sourceTitle(s.source)}</span>
            {#if s.game_filter}<span class="badge warn">game</span>{/if}
            {#if s.score != null}<span class="badge ok">{s.score}%</span>{/if}
            {#if isRunning(s)}
              <span class="badge ok">{t("common.running")}</span>
            {:else if isSelected(s)}
              <span class="badge muted">{t("strategies.activeForEngine")}</span>
            {/if}
          </div>
          {#if s.description}
            <div class="sub">{s.description}</div>
          {/if}
          {#if s.tags.length}
            <div class="row small dim" style="margin-top:6px">
              {#each s.tags as t (t)}<span>#{t}</span>{/each}
            </div>
          {/if}

          {#if expanded === s.id}
            <div style="margin-top:14px" class="list">
              {#if s.options.length}
                {#each s.options as o (o.variable)}
                  <label class="field">
                    {o.title}
                    <select
                      value={optionValue(s, o.variable)}
                      onchange={(e) => store.setOption(s.id, o.variable, Number((e.target as HTMLSelectElement).value))}
                    >
                      <option value={-1}>{t("strategies.defaultOption", { value: o.default_value })}</option>
                      {#each o.values as v, i (i)}
                        <option value={i}>{v.length > 110 ? v.slice(0, 110) + "…" : v}</option>
                      {/each}
                    </select>
                  </label>
                {/each}
              {/if}
              {#if s.flags.length}
                {#each s.flags as f (f.key)}
                  <label class="check">
                    <input
                      type="checkbox"
                      checked={flagValue(s, f.key, f.default)}
                      onchange={(e) => store.setFlag(s.id, f.key, (e.target as HTMLInputElement).checked)}
                    />
                    {f.title}
                  </label>
                {/each}
              {/if}
              <div>
                <div class="small dim" style="margin-bottom:5px">{t("strategies.args")}</div>
                <div class="mono wrap" style="max-height:140px;overflow:auto;color:#9fb0cf">{s.template}</div>
              </div>
            </div>
          {/if}
        </div>

        <div class="row" style="flex:none">
          <button class="btn small" onclick={() => (expanded = expanded === s.id ? null : s.id)}>
            {expanded === s.id ? t("strategies.collapse") : t("strategies.params")}
          </button>
          <button
            class="btn small"
            onclick={() => apply(s)}
            disabled={store.busy || (isSelected(s) && currentEngine === s.engine)}
          >
            {isRunning(s)
              ? t("strategies.running")
              : isSelected(s)
                ? currentEngine === s.engine
                  ? t("strategies.selected")
                  : t("strategies.switchEngine")
                : t("common.apply")}
          </button>
          {#if s.source === "auto" || s.source === "user"}
            <button
              class="btn small danger"
              title={t("common.delete")}
              onclick={() => (pendingDelete = s)}
            >
              <Icon name="trash" size={14} />
            </button>
          {/if}
        </div>
      </div>
    {/each}
  </div>
</div>

{#if pendingDelete}
  <Modal
    title={t("strategies.deleteTitle")}
    subtitle={pendingDelete.name}
    onclose={() => (pendingDelete = null)}
  >
    <div class="small muted" style="line-height:1.55">
      {t("strategies.deleteText")}
    </div>
    <div class="row" style="justify-content:flex-end;margin-top:16px">
      <button class="btn" onclick={() => (pendingDelete = null)}>{t("common.cancel")}</button>
      <button class="btn danger" onclick={confirmDelete}>
        <Icon name="trash" size={14} /> {t("common.delete")}
      </button>
    </div>
  </Modal>
{/if}
