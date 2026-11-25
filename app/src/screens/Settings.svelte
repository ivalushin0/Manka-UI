<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import Markdown from "../lib/Markdown.svelte";
  import { store } from "../lib/store.svelte";
  import { api } from "../lib/api";

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
      store.notify(enabled ? "Автозапуск включён" : "Автозапуск выключен");
    } catch (e) {
      store.fail(e);
    }
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <div class="page-title">Настройки</div>
      <div class="page-sub">Поведение приложения, цели проверки и данные</div>
    </div>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="shield" size={18} /> Запуск и работа</div>
    <div class="grid-2">
      <label class="field">
        Активный движок
        <select
          value={settings?.engine}
          onchange={(e) => store.saveSettings({ engine: (e.target as HTMLSelectElement).value as "zapret2" | "zapret" })}
        >
          <option value="zapret2">Zapret 2 (winws2)</option>
          <option value="zapret">Zapret legacy (winws)</option>
        </select>
      </label>
      <label class="field">
        Fake SNI для стратегий с подстановкой
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
        Запускать Manka при входе в Windows
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={settings?.close_to_tray ?? true}
          onchange={(e) => store.saveSettings({ close_to_tray: (e.target as HTMLInputElement).checked })}
        />
        Закрывать окно в трей (обход продолжает работать)
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={settings?.start_minimized ?? true}
          onchange={(e) => store.saveSettings({ start_minimized: (e.target as HTMLInputElement).checked })}
        />
        Запускать Manka скрыто — только иконка в трее
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={settings?.debug_logs ?? false}
          onchange={(e) => store.saveSettings({ debug_logs: (e.target as HTMLInputElement).checked })}
        />
        Подробные логи компонентов
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={settings?.block_quic ?? true}
          onchange={(e) => store.saveSettings({ block_quic: (e.target as HTMLInputElement).checked })}
        />
        Блокировать QUIC (надёжнее для YouTube и AI-сайтов)
      </label>
    </div>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="bolt" size={18} /> Цели диагностики на главной</div>
    <div class="row">
      {#each store.data?.target_groups ?? [] as g (g.id)}
        <button class="chip" class:on={settings?.targets.includes(g.id)} onclick={() => toggleGroup(g.id)}>
          {g.title}
        </button>
      {/each}
    </div>
    <label class="field" style="margin-top:14px">
      Свои сайты для проверки (по одному в строке)
      <textarea
        placeholder="example.com"
        value={settings?.custom_targets}
        onchange={(e) => store.saveSettings({ custom_targets: (e.target as HTMLTextAreaElement).value })}
      ></textarea>
    </label>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="refresh" size={18} /> Обновления компонентов</div>
    <div class="grid-2">
      <label class="field">
        Проверять обновления компонентов каждые N часов
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
          <Icon name="refresh" size={15} /> Проверить сейчас
        </button>
      </div>
    </div>
  </div>

  <div class="card">
    <div class="card-title">
      <Icon name="download" size={18} /> Обновления Manka
      <span class="badge muted">v{store.data?.version}</span>
      {#if store.appUpdate}
        <span class="badge ok" style="margin-left:auto">доступна {store.appUpdate.version}</span>
      {/if}
    </div>

    {#if store.appUpdate}
      <div class="update-box">
        <div class="row" style="margin-bottom:8px">
          <span class="badge ok">Новая версия {store.appUpdate.version}</span>
          {#if store.appUpdate.date}
            <span class="small dim">{new Date(store.appUpdate.date).toLocaleDateString("ru-RU")}</span>
          {/if}
        </div>
        {#if store.appUpdate.notes}
          <div class="notes"><Markdown text={store.appUpdate.notes} /></div>
        {/if}
        <div class="row" style="margin-top:12px">
          <button class="btn primary" onclick={() => store.installAppUpdate()} disabled={store.appUpdateInstalling}>
            <Icon name="download" size={15} />
            {store.appUpdateInstalling ? "Скачиваю…" : "Установить и перезапустить"}
          </button>
          <button class="btn ghost small" onclick={() => api.openUrl(store.appUpdate!.releases_url)}>
            <Icon name="external" size={14} /> Страница релиза
          </button>
        </div>
        {#if store.appUpdateInstalling}
          <div style="margin-top:12px">
            <div class="row-between small" style="margin-bottom:6px">
              <span>Загрузка обновления…</span>
              <span class="dim">{updatePc != null ? `${updatePc}%` : ""}</span>
            </div>
            <div class="progress"><span style="width:{updatePc ?? 12}%"></span></div>
          </div>
        {/if}
      </div>
    {/if}

    <div class="grid-2">
      <label class="field">
        GitHub-репозиторий для обновлений
        <input
          type="text"
          placeholder="owner/repo — например, ivalu/manka"
          value={repo}
          onchange={(e) => store.saveSettings({ update_repo: (e.target as HTMLInputElement).value.trim() })}
        />
      </label>
      <label class="field">
        Проверять обновления приложения каждые N часов
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
      Проверять обновления Manka при запуске
    </label>

    <div class="row" style="margin-top:14px">
      <button class="btn" onclick={() => store.checkAppUpdate()} disabled={store.appUpdateChecking || !repo}>
        <Icon name="refresh" size={15} />
        {store.appUpdateChecking ? "Проверяю…" : "Проверить обновления Manka"}
      </button>
      <button
        class="btn ghost small"
        onclick={() => api.openUrl(repo ? `https://github.com/${repo}/releases` : "https://github.com")}
      >
        <Icon name="external" size={14} /> Релизы на GitHub
      </button>
    </div>

    <div class="small dim" style="margin-top:12px;line-height:1.55">
      Обновления скачиваются с GitHub Releases и проверяются цифровой подписью. Репозиторий должен быть публичным;
      релиз собирается GitHub Actions по тегу (см. .github/workflows/release.yml). Без указанного репозитория
      проверка обновлений недоступна.
    </div>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="file" size={18} /> Данные</div>
    <div class="kv">
      <span class="k">Папка данных</span>
      <span class="row">
        <span class="mono wrap grow">{store.data?.data_dir}</span>
        <button class="btn small ghost" onclick={() => api.openDataDir()}><Icon name="external" size={14} /> Открыть</button>
      </span>
      <span class="k">Права</span>
      <span>
        {#if store.elevated}
          <span class="badge ok">администратор</span>
        {:else}
          <span class="badge warn">обычный пользователь</span>
          <button class="btn small" style="margin-left:8px" onclick={() => api.relaunchElevated()}>
            Перезапустить от администратора
          </button>
        {/if}
      </span>
      <span class="k">Версия</span><span class="mono">{store.data?.version}</span>
    </div>
    <div class="small dim" style="margin-top:12px;line-height:1.55">
      Настройки и стратегии хранятся в JSON, компоненты — в подпапке components. Для переноса на другой ПК
      достаточно скопировать всю папку данных.
    </div>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="info" size={18} /> О приложении</div>
    <div class="small muted" style="line-height:1.6">
      Manka для Windows — оркестратор обхода DPI. Движки: Zapret 2 и Zapret (bol-van/zapret-win-bundle, MIT).
      Telegram-прокси: tg-ws-proxy-rs (valnesfjord, MIT). Наборы стратегий: CDPIUI-Store и их авторы.
      Приложение не реализует обход DPI само — оно управляет внешними движками.
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
