<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { store } from "../lib/store.svelte";
  import { fmtTime, percent } from "../lib/utils";
  import type { Engine } from "../lib/types";

  const settings = $derived(store.settings);
  const auto = $derived(store.auto);

  const selected = $derived(settings?.autoselect_targets ?? []);

  let expandedResults = $state<Set<string>>(new Set());
  let expandedHistory = $state<Set<string>>(new Set());
  let expandedBaseline = $state<Set<string>>(new Set());
  let expandedSites = $state<Set<string>>(new Set());
  let showAllHistory = $state(false);
  let historyQuery = $state("");

  function toggleTarget(id: string) {
    if (!settings) return;
    const next = selected.includes(id) ? selected.filter((x) => x !== id) : [...selected, id];
    store.saveSettings({ autoselect_targets: next });
  }

  function resultKey(r: { strategy_id: string; engine: string }): string {
    return `${r.engine}::${r.strategy_id}`;
  }

  const sorted = $derived.by(() => {
    if (!auto) return [];
    return [...auto.results].sort((a, b) => b.percent - a.percent || (a.avg_ms || 1e9) - (b.avg_ms || 1e9));
  });

  const running = $derived(auto?.phase === "baseline" || auto?.phase === "testing");
  const progressPc = $derived(auto && auto.total ? Math.round((auto.current / auto.total) * 100) : 0);
  const baselinePc = $derived.by(() => {
    if (!auto) return 0;
    const ok = auto.baseline.reduce((n, r) => n + r.ok, 0);
    const total = auto.baseline.reduce((n, r) => n + r.total, 0);
    return percent(ok, total);
  });

  const history = $derived.by(() => {
    const runs = store.data?.history ?? [];
    const q = historyQuery.trim().toLowerCase();
    if (!q) return runs;
    return runs.filter(
      (run) =>
        run.results.some((r) => r.name.toLowerCase().includes(q)) ||
        run.targets.some((t) => t.toLowerCase().includes(q)),
    );
  });

  const shownHistory = $derived(showAllHistory ? history : history.slice(0, 4));

  function allResultKeys(): string[] {
    return sorted.map(resultKey);
  }

  function expandAllResults() {
    expandedResults = new Set(allResultKeys());
  }

  function collapseAllResults() {
    expandedResults = new Set();
  }

  function toggleResult(key: string) {
    const next = new Set(expandedResults);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    expandedResults = next;
  }

  function toggleHistory(id: string) {
    const next = new Set(expandedHistory);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expandedHistory = next;
  }

  function toggleBaseline(id: string) {
    const next = new Set(expandedBaseline);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expandedBaseline = next;
  }

  function siteKey(runId: string, strategyId: string): string {
    return `${runId}::${strategyId}`;
  }

  function toggleSites(key: string) {
    const next = new Set(expandedSites);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    expandedSites = next;
  }

  function applyCandidate(r: { engine: string; name: string; template: string; percent: number }) {
    void store.applyAuto({
      engine: r.engine as Engine,
      name: r.name,
      template: r.template,
      percent: r.percent,
    });
  }

  function percentClass(p: number): string {
    return p === 100 ? "ok" : p > 0 ? "warn" : "bad";
  }

  /** Old history entries sometimes stored the raw zapret arguments as the name. */
  function prettyName(r: { name: string; template: string }): string {
    const n = (r.name ?? "").trim();
    if (n && !n.startsWith("-") && n.length <= 96) return n;
    const modes = new Set<string>();
    for (const m of r.template.matchAll(/--(?:lua|dpi)-desync=([a-z_]+)/g)) modes.add(m[1]);
    if (modes.size) {
      const engine = r.template.includes("lua-desync") ? "Zapret 2" : "Zapret";
      return `${engine} · ${[...modes].slice(0, 3).join(" + ")}`;
    }
    const raw = (n || r.template).replace(/\s+/g, " ");
    return raw.length > 88 ? `${raw.slice(0, 88)}…` : raw;
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <div class="page-title">Автоподбор стратегии</div>
      <div class="page-sub">Проверяет стратегии по очереди и выбирает лучшую для вашей сети</div>
    </div>
    {#if running}
      <button class="btn danger" onclick={() => store.cancelAuto()}>
        <Icon name="stop" size={15} /> Остановить
      </button>
    {/if}
  </div>

  <div class="banner">
    <Icon name="info" size={20} />
    <span class="grow">
      На время проверки основной обход выключается: тестовая стратегия обслуживает только проверочные запросы Manka.
      Остальной трафик в эти минуты идёт без обхода.
    </span>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="bolt" size={17} /> Цели проверки</div>
    <div class="row">
      {#each store.data?.target_groups ?? [] as g (g.id)}
        <button class="chip" class:on={selected.includes(g.id)} onclick={() => toggleTarget(g.id)}>
          {g.title}
        </button>
      {/each}
    </div>
    <label class="field" style="margin-top:14px">
      Свои сайты (по одному в строке)
      <textarea
        placeholder="example.com / https://site.ru/page"
        value={settings?.custom_targets ?? ""}
        onchange={(e) => store.saveSettings({ custom_targets: (e.target as HTMLTextAreaElement).value })}
      ></textarea>
    </label>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="sliders" size={17} /> Параметры прогона</div>
    <div class="grid-2">
      <div class="list">
        <label class="check">
          <input
            type="checkbox"
            checked={settings?.autoselect_full ?? false}
            onchange={(e) => store.saveSettings({ autoselect_full: (e.target as HTMLInputElement).checked })}
          />
          Полный перебор (дольше, но тщательнее)
        </label>
        <label class="check">
          <input
            type="checkbox"
            checked={settings?.autoselect_include_store ?? true}
            onchange={(e) => store.saveSettings({ autoselect_include_store: (e.target as HTMLInputElement).checked })}
          />
          Включать стратегии из магазина
        </label>
        <div class="small dim" style="line-height:1.5">
          Быстрый режим — около 20 стратегий, полный — весь каталог (может занять более 10 минут).
        </div>
      </div>
      <div class="list">
        <label class="field">
          Запросов на цель
          <input
            type="number"
            min="1"
            max="5"
            value={settings?.autoselect_requests ?? 2}
            onchange={(e) => store.saveSettings({ autoselect_requests: Number((e.target as HTMLInputElement).value) })}
          />
        </label>
        <label class="field">
          Таймаут запроса, секунд
          <input
            type="number"
            min="3"
            max="30"
            value={settings?.autoselect_timeout ?? 8}
            onchange={(e) => store.saveSettings({ autoselect_timeout: Number((e.target as HTMLInputElement).value) })}
          />
        </label>
        <label class="field">
          Остановиться после N идеальных стратегий
          <input
            type="number"
            min="0"
            max="20"
            value={settings?.autoselect_stop_after_perfect ?? 2}
            onchange={(e) => store.saveSettings({ autoselect_stop_after_perfect: Number((e.target as HTMLInputElement).value) })}
          />
        </label>
      </div>
    </div>

    <div class="row" style="margin-top:16px">
      {#if running}
        <button class="btn danger" onclick={() => store.cancelAuto()}>Остановить подбор</button>
      {:else}
        <button class="btn primary" onclick={() => store.startAuto()} disabled={store.busy || !store.elevated}>
          <Icon name="play" size={15} /> Запустить подбор
        </button>
      {/if}
      {#if !store.elevated}
        <span class="small dim">Нужны права администратора.</span>
      {/if}
    </div>
  </div>

  {#if auto}
    <div class="card">
      <div class="card-title">
        <Icon name="wand" size={17} /> Прогон
        <span class="badge" class:ok={auto.phase === "done"} class:warn={running} class:bad={auto.phase === "error" || auto.phase === "cancelled"}>
          {auto.phase === "done" ? "готово" : auto.phase === "testing" ? "проверка" : auto.phase === "baseline" ? "базовая проверка" : auto.phase}
        </span>
        {#if auto.total}<span class="badge muted">{auto.current}/{auto.total}</span>{/if}
      </div>

      {#if running}
        <div class="small muted" style="margin-bottom:9px">
          {auto.phase === "baseline" ? "Проверяю базовое состояние без обхода…" : `Тестирую: ${auto.current_name || "…"}`}
        </div>
        <div class="progress"><span style="width:{auto.phase === "baseline" ? 3 : progressPc}%"></span></div>
      {/if}

      {#if auto.baseline.length}
        <div class="row small muted" style="margin-top:10px">
          <span>Без обхода: <b>{baselinePc}%</b></span>
          <span>· целей: {auto.baseline.length}</span>
        </div>
      {/if}

      {#if auto.error}
        <div class="banner bad" style="margin-top:12px">{auto.error}</div>
      {/if}
    </div>

    {#if sorted.length}
      <div class="card">
        <div class="card-title">
          <Icon name="check" size={17} /> Результаты
          <span class="badge muted">{sorted.length}</span>
          <div class="row" style="margin-left:auto">
            {#if expandedResults.size < sorted.length}
              <button class="btn small ghost" onclick={expandAllResults}>Развернуть все</button>
            {/if}
            {#if expandedResults.size > 0}
              <button class="btn small ghost" onclick={collapseAllResults}>Свернуть все</button>
            {/if}
          </div>
        </div>
        <div class="list">
          {#each sorted as r (resultKey(r))}
            {@const key = resultKey(r)}
            {@const open = expandedResults.has(key)}
            <div class="list-item result" class:open>
              <button class="result-head" aria-expanded={open} onclick={() => toggleResult(key)}>
                <span class="chev" class:rot={open}><Icon name="chevron" size={14} /></span>
                <span class="badge {percentClass(r.percent)}">{r.percent}%</span>
                <span class="grow" style="min-width:0">
                  <span class="name">{prettyName(r)}</span>
                  <span class="sub" style="display:block">
                    {r.engine === "zapret2" ? "Zapret 2" : "Zapret"} · {r.ok}/{r.total} запросов
                    {#if r.avg_ms} · {r.avg_ms} мс{/if}
                    {#if r.start_error} · <span style="color:#ffb4b4">не запустилась</span>{/if}
                  </span>
                </span>
              </button>
              <button
                class="btn small primary"
                disabled={r.percent === 0 || running}
                onclick={() => applyCandidate(r)}
              >
                Применить
              </button>
            </div>
            {#if open}
              <div class="result-body">
                {#if r.start_error}
                  <div class="small" style="color:#ffb4b4;margin-bottom:8px">{r.start_error}</div>
                {/if}
                {#if r.sites.length}
                  <div class="sites">
                    {#each r.sites as site (site.url)}
                      <div class="site">
                        <span class="mono wrap grow">{site.url}</span>
                        <span class="badge {site.ok === site.total ? 'ok' : site.ok ? 'warn' : 'bad'}">{site.ok}/{site.total}</span>
                        {#if site.avg_ms}<span class="small dim">{site.avg_ms} мс</span>{/if}
                      </div>
                    {/each}
                  </div>
                {/if}
                <div class="argline mono wrap">{r.template}</div>
              </div>
            {/if}
          {/each}
        </div>
      </div>
    {/if}
  {/if}

  {#if store.data?.history?.length}
    <div class="card">
      <div class="card-title">
        <Icon name="clock" size={17} /> История подборов
        <span class="badge muted">{history.length}</span>
        <div class="search" style="margin-left:auto;max-width:260px">
          <Icon name="search" size={14} />
          <input type="text" placeholder="Найти стратегию…" bind:value={historyQuery} />
        </div>
      </div>

      {#if !history.length}
        <div class="empty">Ничего не найдено по запросу «{historyQuery}».</div>
      {:else}
        <div class="list">
          {#each shownHistory as run (run.id)}
            {@const open = expandedHistory.has(run.id)}
            <div class="history-run" class:open>
              <button class="history-head" aria-expanded={open} onclick={() => toggleHistory(run.id)}>
                <span class="chev" class:rot={open}><Icon name="chevron" size={14} /></span>
                <span class="grow" style="min-width:0">
                  <span class="name">{fmtTime(run.time)}</span>
                  <span class="sub" style="display:block">
                    {run.tested} стратегий · без обхода {run.baseline_ok}/{run.baseline_total}
                    ({run.baseline_total ? percent(run.baseline_ok, run.baseline_total) : 0}%)
                    · лучшая {run.results[0]?.percent ?? 0}%
                  </span>
                </span>
                <span class="badge muted">{run.engine ?? "—"}</span>
                {#if run.results[0]}
                  <span class="badge {percentClass(run.results[0].percent)}">{run.results[0].percent}%</span>
                {/if}
              </button>

              {#if open}
                <div class="history-body">
                  <div class="row small dim" style="margin-bottom:10px">
                    <span>Целей: {run.targets.length}</span>
                    <span>· проверено: {run.tested}</span>
                    <span>· результатов сохранено: {run.results.length}</span>
                  </div>

                  {#if run.baseline && run.baseline.length}
                    {@const bopen = expandedBaseline.has(run.id)}
                    <div class="baseline">
                      <div class="row-between">
                        <span class="small muted">
                          Без обхода: <b>{run.baseline_ok}/{run.baseline_total}</b>
                          ({run.baseline_total ? percent(run.baseline_ok, run.baseline_total) : 0}%)
                        </span>
                        <button class="btn small ghost" onclick={() => toggleBaseline(run.id)}>
                          {bopen ? "Скрыть сайты" : "Какие сайты не открылись"}
                        </button>
                      </div>
                      {#if bopen}
                        <div class="site-table">
                          {#each run.baseline as site (site.url)}
                            <div class="site">
                              <span class="badge {site.ok === site.total ? 'ok' : site.ok ? 'warn' : 'bad'}">{site.ok}/{site.total}</span>
                              <span class="mono wrap grow">{site.url}</span>
                              {#if site.status}<span class="small dim">HTTP {site.status}</span>{/if}
                              {#if site.error}<span class="small" style="color:#ffb4b4">{site.error}</span>{/if}
                            </div>
                          {/each}
                        </div>
                      {/if}
                    </div>
                  {:else}
                    <div class="small dim" style="margin-bottom:10px">
                      Для этого прогона детали базовой проверки не сохранялись.
                    </div>
                  {/if}

                  {#if run.targets.length}
                    <div class="row small" style="margin-bottom:10px">
                      {#each run.targets.slice(0, 8) as t (t)}
                        <span class="badge muted mono">{t.replace(/^https?:\/\//, "").slice(0, 40)}</span>
                      {/each}
                      {#if run.targets.length > 8}<span class="small dim">+{run.targets.length - 8}</span>{/if}
                    </div>
                  {/if}
                  <div class="hist-table">
                    {#each run.results as r (resultKey(r))}
                      {@const skey = siteKey(run.id, r.strategy_id)}
                      {@const sopen = expandedSites.has(skey)}
                      <div class="hist-result">
                        <div class="hist-row">
                          <span class="badge {percentClass(r.percent)}">{r.percent}%</span>
                          <div class="grow" style="min-width:0">
                            <div class="name" title={r.name}>{prettyName(r)}</div>
                            <div class="sub">
                              {r.engine === "zapret2" ? "Zapret 2" : "Zapret"} · {r.ok}/{r.total}
                              {#if r.avg_ms} · {r.avg_ms} мс{/if}
                              {#if r.start_error} · <span style="color:#ffb4b4">не запустилась</span>{/if}
                            </div>
                          </div>
                          {#if r.sites.length}
                            <button class="btn small ghost" onclick={() => toggleSites(skey)}>
                              {sopen ? "Свернуть сайты" : "Сайты"}
                            </button>
                          {/if}
                          <button
                            class="btn small"
                            disabled={r.percent === 0 || running}
                            onclick={() => applyCandidate(r)}
                          >
                            Применить
                          </button>
                        </div>
                        {#if sopen}
                          <div class="site-table hist-sites">
                            {#each r.sites as site (site.url)}
                              <div class="site">
                                <span class="badge {site.ok === site.total ? 'ok' : site.ok ? 'warn' : 'bad'}">{site.ok}/{site.total}</span>
                                <span class="mono wrap grow">{site.url}</span>
                                {#if site.avg_ms}<span class="small dim">{site.avg_ms} мс</span>{/if}
                                {#if site.error}<span class="small" style="color:#ffb4b4">{site.error}</span>{/if}
                              </div>
                            {/each}
                          </div>
                        {/if}
                      </div>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
          {/each}
        </div>

        {#if history.length > 4}
          <div class="row" style="margin-top:10px;justify-content:center">
            <button class="btn small ghost" onclick={() => (showAllHistory = !showAllHistory)}>
              {showAllHistory ? "Свернуть историю" : `Показать все (${history.length})`}
            </button>
          </div>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .result {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px 4px 4px;
  }

  .result.open {
    border-color: rgba(124, 92, 255, 0.45);
    background: rgba(124, 92, 255, 0.07);
  }

  .result-head,
  .history-head {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
    padding: 8px;
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }

  .chev {
    color: var(--dim);
    display: grid;
    place-items: center;
    transition: transform 0.16s ease;
    flex: none;
  }

  .chev.rot {
    transform: rotate(90deg);
    color: var(--accent-2);
  }

  .result-body {
    margin: -2px 8px 8px 44px;
    padding: 10px 12px;
    border-left: 2px solid rgba(124, 92, 255, 0.4);
    background: rgba(8, 13, 22, 0.45);
    border-radius: 0 10px 10px 0;
    animation: reveal 0.14s ease;
  }

  .sites {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 6px 14px;
    margin-bottom: 10px;
  }

  .site {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .argline {
    background: #070b13;
    border: 1px solid var(--line-soft);
    border-radius: 8px;
    padding: 9px 11px;
    color: #9fb0cf;
    max-height: 130px;
    overflow: auto;
    font-size: 11px;
  }

  .history-run {
    border: 1px solid var(--line-soft);
    border-radius: 12px;
    background: rgba(15, 21, 34, 0.55);
    overflow: hidden;
  }

  .history-run.open {
    border-color: #2a3a59;
    background: rgba(17, 25, 41, 0.8);
  }

  .history-body {
    border-top: 1px solid var(--line-soft);
    padding: 12px;
    animation: reveal 0.16s ease;
  }

  .hist-table {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .hist-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border: 1px solid var(--line-soft);
    border-radius: 9px;
    background: rgba(9, 14, 24, 0.45);
  }

  .baseline {
    border: 1px solid var(--line-soft);
    border-radius: 10px;
    padding: 10px 12px;
    margin-bottom: 10px;
    background: rgba(9, 14, 24, 0.4);
  }

  .site-table {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin-top: 8px;
  }

  .hist-result {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .hist-sites {
    margin-left: 10px;
    padding: 8px 10px;
    border-left: 2px solid rgba(124, 92, 255, 0.4);
    background: rgba(7, 11, 19, 0.45);
    border-radius: 0 8px 8px 0;
    max-height: 220px;
    overflow: auto;
  }

  @keyframes reveal {
    from {
      opacity: 0;
      transform: translateY(-3px);
    }
  }
</style>
