import { api } from "./api";
import { setLanguage, t } from "./i18n.svelte";
import type {
  AppData,
  AppUpdateInfo,
  AutoState,
  CandidateResult,
  DoctorReport,
  Engine,
  ListsReport,
  PortOwner,
  Progress,
  Settings,
  SiteResult,
  StoreItem,
  Strategy,
  TgwsCheck,
  WatchdogReport,
} from "./types";

export type Screen =
  | "home"
  | "strategies"
  | "auto"
  | "components"
  | "telegram"
  | "doctor"
  | "settings"
  | "logs";

function message(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

class Store {
  data = $state<AppData | null>(null);
  screen = $state<Screen>("home");
  loading = $state(true);
  busy = $state(false);
  error = $state<string | null>(null);
  notice = $state<string | null>(null);

  progress = $state<Record<string, Progress>>({});
  auto = $state<AutoState | null>(null);
  autoBusy = $state(false);

  storeItems = $state<StoreItem[]>([]);
  storeLoading = $state(false);
  componentsCheckedAt = $state(0);
  componentsChecking = $state(false);

  serviceResults = $state<SiteResult[] | null>(null);
  diagnosticsBusy = $state(false);

  tgwsCheck = $state<TgwsCheck | null>(null);
  tgwsChecking = $state(false);
  portOwner = $state<PortOwner | null>(null);

  catalogOpen = $state(false);
  catalogEngine = $state<Engine>("zapret2");
  catalogFull = $state(false);
  catalogList = $state<Strategy[]>([]);

  appUpdate = $state<AppUpdateInfo | null>(null);
  appUpdateChecking = $state(false);
  appUpdateInstalling = $state(false);
  appUpdateProgress = $state<{ downloaded: number; total: number | null } | null>(null);

  doctorReport = $state<DoctorReport | null>(null);
  doctorBusy = $state(false);
  doctorExporting = $state(false);
  listsBusy = $state(false);
  watchdogBusy = $state(false);

  private noticeTimer: number | undefined;

  get settings(): Settings | null {
    return this.data?.settings ?? null;
  }

  get engineRunning(): boolean {
    return this.data?.engine.running ?? false;
  }

  get activeStrategy(): Strategy | null {
    const s = this.data?.settings;
    if (!s) return null;
    const id = s.active[s.engine];
    if (!id) return null;
    return this.data?.strategies.find((x) => x.id === id) ?? null;
  }

  get strategies(): Strategy[] {
    return this.data?.strategies ?? [];
  }

  get elevated(): boolean {
    return this.data?.elevated ?? false;
  }

  async init() {
    await this.refresh();
  }

  async refresh() {
    try {
      this.data = await api.getState();
      setLanguage(this.data.settings.language || "ru");
    } catch (e) {
      this.fail(e);
    } finally {
      this.loading = false;
    }
  }

  notify(text: string) {
    this.notice = text;
    window.clearTimeout(this.noticeTimer);
    this.noticeTimer = window.setTimeout(() => (this.notice = null), 4200);
  }

  fail(e: unknown) {
    this.error = message(e);
    window.setTimeout(() => {
      if (this.error === message(e)) this.error = null;
    }, 9000);
  }

  async guard<T>(fn: () => Promise<T>): Promise<T | undefined> {
    this.busy = true;
    try {
      return await fn();
    } catch (e) {
      this.fail(e);
      return undefined;
    } finally {
      this.busy = false;
    }
  }

  async togglePower() {
    const s = this.settings;
    if (!s) return;
    await this.guard(async () => {
      if (this.data?.engine.running) {
        await api.engineStop();
        this.notify(t("store.bypassStopped"));
      } else {
        await api.engineStart();
        this.notify(t("store.bypassStarted"));
      }
      await this.refresh();
    });
  }

  async saveSettings(patch: Partial<Settings>) {
    const s = this.settings;
    if (!s) return;
    const prev = s;
    const next = { ...s, ...patch };
    // optimistic update with rollback: the UI must never drift from the backend
    if (this.data) this.data.settings = next;
    if (patch.language !== undefined) setLanguage(patch.language);
    try {
      const saved = await api.saveSettings(next);
      if (this.data) this.data.settings = saved;
      setLanguage(saved.language || "ru");
    } catch (e) {
      if (this.data) this.data.settings = prev;
      if (patch.language !== undefined) setLanguage(prev.language || "ru");
      this.fail(e);
    }
  }

  async applyStrategy(id: string) {
    const s = this.settings;
    if (!s) return;
    await this.guard(async () => {
      const saved = await api.setActiveStrategy(s.engine, id);
      if (this.data) this.data.settings = saved;
      await this.refresh();
      this.notify(t("store.strategyApplied"));
    });
  }

  async setOption(strategyId: string, variable: string, index: number) {
    await this.guard(async () => {
      const saved = await api.setStrategyOption(strategyId, variable, index);
      if (this.data) this.data.settings = saved;
    });
  }

  async setFlag(strategyId: string, key: string, value: boolean) {
    await this.guard(async () => {
      const saved = await api.setStrategyFlag(strategyId, key, value);
      if (this.data) this.data.settings = saved;
      await this.refreshStatus();
    });
  }

  async installComponent(id: string, force = false) {
    await this.guard(async () => {
      await api.installComponent(id, force);
      await this.refresh();
      this.notify(t("store.componentInstalled"));
    });
  }

  async updateComponent(id: string) {
    await this.installComponent(id, true);
  }

  async checkUpdates(silent = false) {
    if (this.componentsChecking) return;
    this.componentsChecking = true;
    try {
      await api.checkUpdates();
      await this.refresh();
      this.componentsCheckedAt = Date.now();
      const updates = this.data?.components.filter((c) => c.update_available) ?? [];
      if (!silent) {
        this.notify(
          updates.length
            ? t("store.updatesAvailable", { list: updates.map((u) => u.name).join(", ") })
            : t("store.allUpToDate"),
        );
      }
    } catch (e) {
      if (!silent) this.fail(e);
    } finally {
      this.componentsChecking = false;
    }
  }

  async checkAppUpdate(silent = false) {
    const repo = this.settings?.update_repo?.trim();
    if (!repo) {
      if (!silent) this.fail(t("store.needRepo"));
      return;
    }
    this.appUpdateChecking = true;
    try {
      const found = await api.appUpdateCheck();
      this.appUpdate = found;
      if (!silent || found) {
        this.notify(
          found ? t("store.appUpdateFound", { version: found.version }) : t("store.appUpToDate"),
        );
      }
    } catch (e) {
      if (!silent) this.fail(e);
    } finally {
      this.appUpdateChecking = false;
    }
  }

  async installAppUpdate() {
    this.appUpdateInstalling = true;
    this.appUpdateProgress = { downloaded: 0, total: null };
    try {
      await api.appUpdateInstall();
    } catch (e) {
      this.fail(e);
      this.appUpdateInstalling = false;
    }
  }

  onAppUpdateProgress(payload: { downloaded: number; total: number | null }) {
    this.appUpdateProgress = payload;
  }

  async rollbackComponent(id: string) {
    await this.guard(async () => {
      await api.rollbackComponent(id);
      await this.refresh();
      this.notify(id === "zapret" ? t("store.rollbackZapret") : t("store.rollbackTgws"));
    });
  }

  async toggleProxy() {
    const s = this.settings;
    if (!s) return;
    if (this.data?.proxy.running) {
      await this.stopProxy();
    } else {
      await this.startProxy();
    }
  }

  async startProxy() {
    const s = this.settings;
    if (!s) return;
    try {
      const owner = await api.portOwner(s.tgws_port);
      if (owner) {
        this.portOwner = owner;
        this.screen = "telegram";
        this.notify(t("store.portBusy", { port: s.tgws_port, name: owner.name, pid: owner.pid }));
        return;
      }
    } catch {
      /* port check is best effort */
    }
    await this.guard(async () => {
      await api.tgwsStart();
      await this.refresh();
      this.portOwner = null;
      this.notify(t("store.proxyStarted"));
    });
  }

  async stopProxy() {
    await this.guard(async () => {
      await api.tgwsStop();
      await this.refresh();
      this.notify(t("store.proxyStopped"));
    });
  }

  async refreshStatus() {
    if (!this.data) return;
    const wasEngine = this.data.engine.running;
    const wasProxy = this.data.proxy.running;
    try {
      const st = await api.getStatus();
      this.data.engine = st.engine;
      this.data.proxy = st.proxy;
      if (wasEngine && !st.engine.running && this.settings?.enabled) {
        this.notify(t("store.engineStopped"));
      }
      if (wasProxy && !st.proxy.running && this.settings?.tgws_enabled) {
        this.notify(t("store.proxyStoppedUnexpected"));
      }
    } catch {
      /* window is closing */
    }
  }

  async checkPort() {
    const port = this.settings?.tgws_port;
    if (!port) return;
    try {
      this.portOwner = await api.portOwner(port);
    } catch {
      this.portOwner = null;
    }
  }

  isKnownProxy(name: string): boolean {
    const n = name.toLowerCase();
    return n.includes("tgws") || n.includes("tg-ws-proxy") || n.includes("tgwsproxy");
  }

  async takePortAndStart() {
    const port = this.settings?.tgws_port;
    if (!port) return;
    await this.guard(async () => {
      await api.stopPortOwner(port);
      this.portOwner = null;
      await api.tgwsStart();
      await this.refresh();
      this.notify(t("store.portTaken"));
    });
  }

  async checkProxy() {
    this.tgwsChecking = true;
    this.tgwsCheck = null;
    try {
      this.tgwsCheck = await api.tgwsCheck();
    } catch (e) {
      this.fail(e);
    } finally {
      this.tgwsChecking = false;
    }
  }

  async regenerateSecret() {
    await this.guard(async () => {
      const saved = await api.tgwsRegenerateSecret();
      if (this.data) this.data.settings = saved;
      this.notify(t("store.secretUpdated"));
    });
  }

  async refreshStore() {
    this.storeLoading = true;
    try {
      await api.storeRefresh();
      this.storeItems = await api.storeItems();
      this.notify(t("store.storeRefreshed"));
    } catch (e) {
      this.fail(e);
    } finally {
      this.storeLoading = false;
    }
  }

  async loadStore(force = false) {
    if (this.storeLoading) return;
    if (this.storeItems.length && !force) return;
    this.storeLoading = true;
    try {
      this.storeItems = await api.storeItems();
      if (!this.storeItems.length) {
        await api.storeRefresh();
        this.storeItems = await api.storeItems();
      }
    } catch (e) {
      this.fail(e);
    } finally {
      this.storeLoading = false;
    }
  }

  async installStoreItem(storeId: string) {
    await this.guard(async () => {
      await api.storeInstall(storeId);
      this.storeItems = await api.storeItems();
      await this.refresh();
      this.notify(t("store.kitInstalled"));
    });
  }

  async removeStoreItem(storeId: string) {
    await this.guard(async () => {
      await api.storeRemove(storeId);
      this.storeItems = await api.storeItems();
      await this.refresh();
      this.notify(t("store.kitRemoved"));
    });
  }

  selectedTargets(): string[] {
    const s = this.settings;
    if (!s) return [];
    return this.targetsFor(s.targets);
  }

  async runDiagnostics() {
    this.diagnosticsBusy = true;
    this.serviceResults = null;
    try {
      this.serviceResults = await api.probeTargets(this.selectedTargets(), 1, 8);
      const ok = this.serviceResults.reduce((n, r) => n + r.ok, 0);
      const total = this.serviceResults.reduce((n, r) => n + r.total, 0);
      if (this.data) {
        this.data.settings.last_check = Date.now();
        this.data.settings.last_check_rate = total ? Math.round((ok / total) * 100) : 0;
      }
      this.notify(t("store.checkDone", { percent: total ? Math.round((ok / total) * 100) : 0 }));
    } catch (e) {
      this.fail(e);
    } finally {
      this.diagnosticsBusy = false;
    }
  }

  async startAuto() {
    const s = this.settings;
    if (!s) return;
    this.autoBusy = true;
    this.auto = null;
    try {
      const ids = s.autoselect_targets.length ? s.autoselect_targets : s.targets;
      const targets = s.autoselect_full ? this.targetsFor(ids) : this.quickTargetsFor(ids);
      this.auto = await api.autoStart({
        engines: [s.engine],
        targets,
        full: s.autoselect_full,
        include_store: s.autoselect_include_store,
        requests: s.autoselect_requests,
        timeout_sec: s.autoselect_timeout,
        stop_after_perfect: s.autoselect_stop_after_perfect,
      });
    } catch (e) {
      this.fail(e);
    } finally {
      this.autoBusy = false;
      await this.refresh();
    }
  }

  targetsFor(ids: string[]): string[] {
    const groups = this.data?.target_groups ?? [];
    const grouped = groups.filter((g) => ids.includes(g.id)).flatMap((g) => g.urls);
    return [...new Set([...grouped, ...this.customTargets()])];
  }

  /** Quick runs check the two most representative URLs per group (plus custom ones). */
  quickTargetsFor(ids: string[]): string[] {
    const groups = this.data?.target_groups ?? [];
    const grouped = groups.filter((g) => ids.includes(g.id)).flatMap((g) => g.urls.slice(0, 2));
    return [...new Set([...grouped, ...this.customTargets()])];
  }

  customTargets(): string[] {
    const out: string[] = [];
    for (const raw of (this.settings?.custom_targets ?? "").split(/\r?\n/)) {
      const line = raw.trim();
      if (!line || line.startsWith("#")) continue;
      out.push(line.startsWith("http://") || line.startsWith("https://") ? line : `https://${line}/`);
    }
    return out;
  }

  async cancelAuto() {
    try {
      await api.autoCancel();
      this.notify(t("store.autoStopping"));
    } catch (e) {
      this.fail(e);
    }
  }

  async applyAuto(candidate: Pick<CandidateResult, "engine" | "name" | "template" | "percent">) {
    await this.guard(async () => {
      await api.autoApply(candidate);
      await this.refresh();
      this.notify(t("store.bestApplied"));
      this.screen = "home";
    });
  }

  async loadCatalog() {
    try {
      this.catalogList = await api.catalog(this.catalogEngine, this.catalogFull);
    } catch (e) {
      this.fail(e);
    }
  }

  onProgress(p: Progress) {
    this.progress = { ...this.progress, [p.id]: p };
    if (p.percent >= 100) {
      window.setTimeout(() => {
        const next = { ...this.progress };
        delete next[p.id];
        this.progress = next;
      }, 2500);
    }
  }

  onAuto(state: AutoState) {
    this.auto = state;
  }

  async loadLogs(name: string, lines = 400): Promise<string[]> {
    try {
      return await api.readLog(name, lines);
    } catch (e) {
      this.fail(e);
      return [];
    }
  }

  // ---- Doctor / lists / watchdog

  async runDoctor() {
    this.doctorBusy = true;
    try {
      this.doctorReport = await api.doctorRun();
    } catch (e) {
      this.fail(e);
    } finally {
      this.doctorBusy = false;
    }
  }

  async exportDoctor() {
    this.doctorExporting = true;
    try {
      const path = await api.doctorExport();
      this.notify(t("store.doctorSaved", { path }));
      return path;
    } catch (e) {
      this.fail(e);
      return null;
    } finally {
      this.doctorExporting = false;
    }
  }

  async updateLists() {
    this.listsBusy = true;
    try {
      const report = await api.listsUpdateNow();
      this.notify(t("store.listsUpdated", { count: report.domains }));
      await this.refresh();
    } catch (e) {
      this.fail(e);
    } finally {
      this.listsBusy = false;
    }
  }

  async watchdogCheckNow() {
    this.watchdogBusy = true;
    try {
      const report = await api.watchdogCheckNow();
      this.notify(
        report.checked
          ? report.healthy
            ? t("store.watchdogOk", { percent: report.percent })
            : t("store.watchdogProblem", { detail: report.detail })
          : t("store.watchdogSkipped"),
      );
      await this.refresh();
    } catch (e) {
      this.fail(e);
    } finally {
      this.watchdogBusy = false;
    }
  }

  onWatchdog(report: WatchdogReport) {
    if (report.checked && !report.healthy) {
      this.notify(t("store.watchdogDegraded", { percent: report.percent }));
    }
    void this.refresh();
  }

  onWatchdogFixed(payload: { name: string; percent: number }) {
    this.notify(t("store.watchdogFixed", { name: payload.name, percent: payload.percent }));
    void this.refresh();
  }

  onWatchdogFailed() {
    this.notify(t("store.watchdogFailed"));
    void this.refresh();
  }

  onListsUpdated(report: ListsReport) {
    this.notify(t("store.listsUpdated", { count: report.domains }));
    void this.refresh();
  }
}

export const store = new Store();
