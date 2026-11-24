<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../lib/Icon.svelte";
  import Modal from "../lib/Modal.svelte";
  import Markdown from "../lib/Markdown.svelte";
  import { store } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { cleanMarkdown, fmtTime } from "../lib/utils";
  import type { ComponentStatus, StoreItem } from "../lib/types";

  onMount(() => {
    void store.loadStore();
    if (Date.now() - store.componentsCheckedAt > 10 * 60_000) {
      void store.checkUpdates(true);
    }
  });

  const components = $derived(store.data?.components ?? []);
  const dpiComponents = $derived(components.filter((c) => c.kind !== "proxy"));
  const proxyComponents = $derived(components.filter((c) => c.kind === "proxy"));
  const installedDpi = $derived(dpiComponents.filter((c) => c.installed));
  const availableDpi = $derived(dpiComponents.filter((c) => !c.installed));

  let query = $state("");
  let kind = $state<"all" | "configlist" | "lsubscription">("all");
  let detail = $state<StoreItem | null>(null);
  let componentDetail = $state<ComponentStatus | null>(null);

  const storeItems = $derived(store.storeItems.filter((i) => i.kind !== "component"));
  const unsupported = $derived(
    store.storeItems.filter(
      (i) =>
        i.kind === "component" &&
        !/zapret|tg[\s_-]?ws/i.test(`${i.name} ${i.short_name} ${i.store_id}`),
    ),
  );

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return storeItems
      .filter((i) => kind === "all" || i.kind === kind)
      .filter((i) => {
        if (!q) return true;
        return (
          i.name.toLowerCase().includes(q) ||
          i.short_name.toLowerCase().includes(q) ||
          i.developer.toLowerCase().includes(q) ||
          i.description.toLowerCase().includes(q)
        );
      })
      .sort((a, b) => {
        const ai = a.installed_version ? 0 : 1;
        const bi = b.installed_version ? 0 : 1;
        return ai - bi || (a.short_name || a.name).localeCompare(b.short_name || b.name, "ru");
      });
  });

  const counts = $derived.by(() => {
    let installed = 0;
    for (const i of storeItems) if (i.installed_version) installed++;
    return {
      all: storeItems.length,
      configlist: storeItems.filter((i) => i.kind === "configlist").length,
      lsubscription: storeItems.filter((i) => i.kind === "lsubscription").length,
      installed,
    };
  });

  const storeProgress = $derived(
    Object.values(store.progress).find((p) => p.id !== "zapret" && p.id !== "tgws") ?? null,
  );

  function plural(n: number, one: string, few: string, many: string): string {
    const m10 = n % 10;
    const m100 = n % 100;
    if (m10 === 1 && m100 !== 11) return `${n} ${one}`;
    if (m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14)) return `${n} ${few}`;
    return `${n} ${many}`;
  }

  function version(v: string | null | undefined): string {
    if (!v) return "—";
    return `v${v.replace(/^v/i, "")}`;
  }

  function installedLabel(v: string | null | undefined): string {
    if (!v) return "установлен";
    return v.toLowerCase() === "installed" ? "установлен" : `установлен ${v}`;
  }

  function checkLabel(c: ComponentStatus): string {
    if (!c.installed) return "не установлен";
    if (c.update_available) return `есть ${c.latest ?? "новее"}`;
    return store.componentsCheckedAt ? "актуально" : "не проверено";
  }

  function note(c: ComponentStatus): string {
    if (c.id === "tgws") {
      return `MTProto-прокси без трея, 127.0.0.1:${store.settings?.tgws_port ?? 1443}`;
    }
    return c.note ?? "";
  }

  function progressFor(id: string) {
    return store.progress[id] ?? null;
  }

  function preview(text: string): string {
    const clean = cleanMarkdown(text);
    return clean.length > 230 ? `${clean.slice(0, 230)}…` : clean;
  }

  async function installItem(item: StoreItem) {
    await store.installStoreItem(item.store_id);
    detail = null;
  }

  async function removeItem(item: StoreItem) {
    await store.removeStoreItem(item.store_id);
    detail = null;
  }

  async function updateComponent(id: string) {
    await store.updateComponent(id);
    componentDetail = null;
  }
</script>

{#snippet componentCard(c)}
  {@const p = progressFor(c.id)}
  <div class="card comp" class:installed={c.installed} class:available={!c.installed}>
    <div class="card-title">
      <span class="comp-icon" class:on={c.installed}>
        <Icon name={c.id === "tgws" ? "send" : "shield"} size={20} />
      </span>
      <div class="grow" style="min-width:0">
        <div>{c.name}</div>
        <div class="small dim" style="font-weight:400">{note(c)}</div>
      </div>
      <span class="badge" class:ok={c.installed} class:muted={!c.installed}>
        {c.installed ? version(c.version) : "не установлен"}
      </span>
    </div>
    {#if c.installed}
      <div style="margin-bottom:10px">
        <span class="badge" class:ok={!c.update_available} class:warn={c.update_available}>{checkLabel(c)}</span>
      </div>
    {/if}

    {#if p}
      <div class="row-between small" style="margin-bottom:6px">
        <span>{p.stage}</span>
        <span class="dim">{p.percent}%</span>
      </div>
      <div class="progress" style="margin-bottom:12px"><span style="width:{p.percent}%"></span></div>
    {/if}

    {#if c.update_available}
      <div class="banner" style="padding:8px 11px;margin-bottom:10px">
        <Icon name="download" size={15} />
        <span class="grow small">Доступно обновление {c.latest}</span>
      </div>
    {/if}

    <div class="row">
      {#if !c.installed}
        <button class="btn primary" onclick={() => store.installComponent(c.id)} disabled={store.busy || !!p}>
          <Icon name="download" size={15} /> Установить
        </button>
      {:else if c.update_available}
        <button class="btn primary" onclick={() => store.updateComponent(c.id)} disabled={store.busy || !!p}>
          <Icon name="refresh" size={15} /> Обновить до {c.latest}
        </button>
      {:else}
        <button class="btn small ghost" onclick={() => store.updateComponent(c.id)} disabled={store.busy || !!p}>
          <Icon name="refresh" size={14} /> Переустановить
        </button>
      {/if}
      <button class="btn small ghost" onclick={() => (componentDetail = c)}>Подробнее</button>
      {#if c.id === "zapret" && c.installed}
        <button class="btn small ghost" onclick={() => store.rollbackZapret()} disabled={store.busy}>Откатить</button>
      {/if}
    </div>
  </div>
{/snippet}

<div class="page">
  <div class="page-head">
    <div>
      <div class="page-title">Компоненты</div>
      <div class="page-sub">Движки, Telegram-прокси и наборы стратегий из магазина CDPI</div>
    </div>
    <div class="row">
      {#if store.componentsChecking}
        <span class="small dim">Проверяю обновления…</span>
      {:else if store.componentsCheckedAt}
        <span class="small dim">Проверено {fmtTime(store.componentsCheckedAt)}</span>
      {/if}
      <button class="btn small" onclick={() => store.checkUpdates()} disabled={store.componentsChecking}>
        <Icon name="refresh" size={14} /> {store.componentsChecking ? "Проверяю…" : "Проверить обновления"}
      </button>
    </div>
  </div>

  <div class="section-label">Установленные движки DPI</div>
  {#if installedDpi.length}
    <div class="grid-2">
      {#each installedDpi as c (c.id)}
        {@render componentCard(c)}
      {/each}
    </div>
  {:else}
    <div class="empty">
      Движок DPI не установлен. Выберите доступный движок ниже и нажмите «Установить».
    </div>
  {/if}

  {#if availableDpi.length}
    <div class="section-label">Доступные для установки</div>
    <div class="grid-2">
      {#each availableDpi as c (c.id)}
        {@render componentCard(c)}
      {/each}
    </div>
  {/if}

  <div class="section-label">Telegram-прокси</div>
  <div class="grid-2">
    {#each proxyComponents as c (c.id)}
      {@render componentCard(c)}
    {/each}
  </div>

  <div class="card">
    <div class="card-title">
      <Icon name="package" size={18} /> Магазин CDPI
      <span class="badge muted">
        {plural(counts.all, "набор", "набора", "наборов")} · установлено {counts.installed}
      </span>
      <button class="btn small ghost" style="margin-left:auto" onclick={() => store.refreshStore()} disabled={store.storeLoading}>
        {store.storeLoading ? "Обновляю…" : "Обновить каталог"}
      </button>
    </div>

    <div class="store-toolbar">
      <div class="tabs">
        <button class:on={kind === "all"} onclick={() => (kind = "all")}>Все <span class="dim">{counts.all}</span></button>
        <button class:on={kind === "configlist"} onclick={() => (kind = "configlist")}>
          Наборы <span class="dim">{counts.configlist}</span>
        </button>
        <button class:on={kind === "lsubscription"} onclick={() => (kind = "lsubscription")}>
          Списки <span class="dim">{counts.lsubscription}</span>
        </button>
      </div>
      <div class="search grow">
        <Icon name="search" size={15} />
        <input type="text" placeholder="Поиск по названию, автору, описанию…" bind:value={query} />
      </div>
      <span class="badge muted">{filtered.length}</span>
    </div>

    {#if unsupported.length}
      <div class="small dim" style="margin:-4px 0 10px;line-height:1.5">
        Движки из каталога CDPI (ByeDPI, GoodbyeDPI, NoDPI, SpoofDPI) Manka пока не поддерживает — они
        показаны ниже только для справки. Zapret 2 и TG WS Proxy управляются выше.
      </div>
    {/if}

    {#if storeProgress}
      <div class="row-between small" style="margin-bottom:6px">
        <span>{storeProgress.stage}</span>
        <span class="dim">{storeProgress.percent}%</span>
      </div>
      <div class="progress" style="margin-bottom:12px"><span style="width:{storeProgress.percent}%"></span></div>
    {/if}

    {#if store.storeLoading && !storeItems.length}
      <div class="empty">Загружаю каталог магазина…</div>
    {:else if !storeItems.length}
      <div class="empty">
        Каталог пуст. Проверьте подключение к интернету и нажмите «Обновить каталог».
      </div>
    {:else if !filtered.length}
      <div class="empty">Ничего не найдено по запросу «{query}».</div>
    {:else}
      <div class="list">
        {#each filtered as item (item.store_id)}
          {@const short = item.short_name || item.name}
          <div class="list-item store-item">
            <div class="grow" style="min-width:0">
              <div class="row">
                <span class="name">{short}</span>
                {#if item.kind === "lsubscription"}
                  <span class="badge muted">список</span>
                {/if}
                <span class="badge muted">{item.developer || "CDPI"}</span>
                {#if item.installed_version}
                  <span class="badge ok">{installedLabel(item.installed_version)}</span>
                {/if}
              </div>
              {#if item.description}
                <div class="sub desc-preview">{preview(item.description)}</div>
              {/if}
              {#if item.warning}
                <div class="small" style="color:#fcd58b;margin-top:6px">{item.warning}</div>
              {/if}
            </div>
            <div class="row store-actions" style="flex:none">
              <button class="btn small ghost" onclick={() => (detail = item)}>
                Подробнее
              </button>
              <button class="btn small primary" onclick={() => store.installStoreItem(item.store_id)} disabled={store.busy}>
                {item.installed_version ? "Переустановить" : "Установить"}
              </button>
              {#if item.installed_version}
                <button class="btn small danger" onclick={() => store.removeStoreItem(item.store_id)} disabled={store.busy} title="Удалить">
                  <Icon name="trash" size={14} />
                </button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  {#if unsupported.length}
    <div class="card">
      <div class="card-title">
        <Icon name="package" size={17} /> Другие движки CDPI
        <span class="badge muted">{unsupported.length}</span>
      </div>
      <div class="small dim" style="margin-bottom:10px;line-height:1.5">
        Эти компоненты есть в каталоге CDPI, но Manka пока не умеет их запускать. Управляемые движки —
        Zapret 2 и Telegram-прокси выше.
      </div>
      <div class="list">
        {#each unsupported as item (item.store_id)}
          <div class="list-item">
            <div class="grow" style="min-width:0">
              <div class="name">{item.short_name || item.name}</div>
              <div class="sub">{item.developer || "CDPI"}</div>
            </div>
            <span class="badge muted">не поддерживается</span>
            <button class="btn small ghost" onclick={() => (detail = item)}>Подробнее</button>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if store.data?.kits?.length}
    <div class="card">
      <div class="card-title"><Icon name="layers" size={17} /> Установленные наборы</div>
      <div class="list">
        {#each store.data!.kits as kit (kit.id)}
          <div class="list-item">
            <div class="grow">
              <div class="name">{kit.name}</div>
              <div class="sub mono">{kit.id}</div>
            </div>
            <span class="badge">{kit.version}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

{#if componentDetail}
  <Modal
    title={componentDetail.name}
    subtitle={componentDetail.installed ? `Установлен ${version(componentDetail.version)}` : "Компонент не установлен"}
    onclose={() => (componentDetail = null)}
  >
    <div class="kv" style="margin-bottom:16px">
      <span class="k">Состояние</span>
      <span>
        {#if componentDetail.installed}
          <span class="badge ok">установлен</span>
        {:else}
          <span class="badge muted">не установлен</span>
        {/if}
        {#if componentDetail.update_available}
          <span class="badge warn" style="margin-left:6px">доступно {componentDetail.latest}</span>
        {/if}
      </span>
      {#if note(componentDetail)}
        <span class="k">Описание</span><span class="small muted">{note(componentDetail)}</span>
      {/if}
      {#if componentDetail.path}
        <span class="k">Путь</span>
        <span class="mono wrap small">{componentDetail.path}</span>
      {/if}
    </div>
    <div class="row" style="justify-content:flex-end">
      {#if !componentDetail.installed}
        <button class="btn primary" onclick={() => updateComponent(componentDetail.id)} disabled={store.busy}>
          <Icon name="download" size={15} /> Установить
        </button>
      {:else}
        <button class="btn" onclick={() => updateComponent(componentDetail.id)} disabled={store.busy}>
          <Icon name="refresh" size={15} /> {componentDetail.update_available ? "Обновить" : "Переустановить"}
        </button>
      {/if}
      {#if componentDetail.id === "zapret" && componentDetail.installed}
        <button class="btn ghost" onclick={() => store.rollbackZapret()} disabled={store.busy}>Откатить</button>
      {/if}
    </div>
  </Modal>
{/if}

{#if detail}
  <Modal
    title={detail.short_name || detail.name}
    subtitle={`${detail.developer || "CDPI"} · ${detail.kind === "lsubscription" ? "список доменов" : "набор стратегий"}`}
    wide
    onclose={() => (detail = null)}
  >
    <div class="row" style="margin-bottom:12px">
      {#if detail.kind === "component"}
        <span class="badge muted">не поддерживается Manka</span>
      {:else if detail.installed_version}
        <span class="badge ok">{installedLabel(detail.installed_version)}</span>
      {:else}
        <span class="badge muted">не установлен</span>
      {/if}
      {#if detail.version_control}
        <span class="badge muted">{detail.version_control}</span>
      {/if}
      {#if detail.link}
        <button class="btn small ghost" onclick={() => detail && api.openUrl(detail.link)}>
          <Icon name="external" size={14} /> Источник
        </button>
      {/if}
    </div>

    {#if detail.warning}
      <div class="banner" style="margin-bottom:12px">
        <Icon name="info" size={18} />
        <span class="grow small">{detail.warning}</span>
      </div>
    {/if}

    <div class="detail-scroll">
      {#if detail.description}
        <Markdown text={detail.description} />
      {:else}
        <div class="empty">Описание не указано.</div>
      {/if}
    </div>

    <div class="row" style="justify-content:flex-end;margin-top:16px">
      {#if detail.kind === "component"}
        <button class="btn" onclick={() => (detail = null)}>Закрыть</button>
      {:else}
        {#if detail.installed_version}
          <button class="btn danger" onclick={() => removeItem(detail!)} disabled={store.busy}>
            <Icon name="trash" size={14} /> Удалить
          </button>
        {/if}
        <button class="btn primary" onclick={() => installItem(detail!)} disabled={store.busy}>
          <Icon name="download" size={15} /> {detail.installed_version ? "Переустановить" : "Установить"}
        </button>
      {/if}
    </div>
  </Modal>
{/if}

<style>
  .comp .card-title {
    align-items: flex-start;
  }

  .comp-icon {
    width: 36px;
    height: 36px;
    border-radius: 11px;
    display: grid;
    place-items: center;
    background: #0f1727;
    color: var(--dim);
    border: 1px solid var(--line-soft);
    flex: none;
  }

  .comp-icon.on {
    color: var(--accent-2);
    border-color: rgba(34, 211, 238, 0.35);
    background: rgba(34, 211, 238, 0.08);
  }

  .comp.available {
    border-style: dashed;
    background: rgba(10, 15, 24, 0.45);
  }

  .comp.available .comp-icon {
    color: var(--muted);
  }

  .section-label {
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.45px;
    text-transform: uppercase;
    color: var(--dim);
    margin: 2px 2px -4px;
  }

  .store-toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    margin-bottom: 14px;
  }

  .store-item {
    align-items: flex-start;
  }

  .desc-preview {
    line-height: 1.5;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .store-actions {
    margin-left: auto;
  }

  .detail-scroll {
    max-height: min(52vh, 520px);
    overflow: auto;
    padding-right: 4px;
  }
</style>
