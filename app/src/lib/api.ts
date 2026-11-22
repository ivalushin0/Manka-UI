import { invoke } from "@tauri-apps/api/core";
import type {
  AppData,
  AppUpdateInfo,
  AutoRequest,
  AutoState,
  CandidateResult,
  ComponentStatus,
  Engine,
  LogInfo,
  Progress,
  Settings,
  SiteResult,
  StatusDto,
  StoreItem,
  Strategy,
  TgwsCheck,
  PortOwner,
} from "./types";

export const api = {
  getState: () => invoke<AppData>("get_state"),
  getStatus: () => invoke<StatusDto>("get_status"),
  portOwner: (port: number) => invoke<PortOwner | null>("port_owner", { port }),
  stopPortOwner: (port: number) => invoke<string>("stop_port_owner", { port }),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  setEnabled: (enabled: boolean) => invoke("set_enabled", { enabled }),
  engineStart: () => invoke("engine_start"),
  engineStop: () => invoke("engine_stop"),
  setActiveStrategy: (engine: Engine, id: string) =>
    invoke<Settings>("set_active_strategy", { engine, id }),
  setStrategyOption: (strategyId: string, variable: string, index: number) =>
    invoke<Settings>("set_strategy_option", { strategyId, variable, index }),
  setStrategyFlag: (strategyId: string, key: string, value: boolean) =>
    invoke<Settings>("set_strategy_flag", { strategyId, key, value }),
  catalog: (engine: Engine, full: boolean) => invoke<Strategy[]>("catalog", { engine, full }),
  installComponent: (id: string, force = false) =>
    invoke<ComponentStatus>("install_component", { id, force }),
  checkUpdates: () => invoke<ComponentStatus[]>("check_updates"),
  rollbackComponent: (id: string) => invoke<ComponentStatus>("rollback_component", { id }),
  tgwsStart: () => invoke("tgws_start"),
  tgwsStop: () => invoke("tgws_stop"),
  tgwsCheck: () => invoke<TgwsCheck>("tgws_check"),
  tgwsRegenerateSecret: () => invoke<Settings>("tgws_regenerate_secret"),
  storeRefresh: () => invoke<number>("store_refresh"),
  storeItems: () => invoke<StoreItem[]>("store_items"),
  storeInstall: (storeId: string) => invoke<string>("store_install", { storeId }),
  storeRemove: (storeId: string) => invoke("store_remove", { storeId }),
  probeTargets: (targets: string[], requests: number, timeoutSec: number) =>
    invoke<SiteResult[]>("probe_targets", { targets, requests, timeoutSec }),
  autoStart: (request: AutoRequest) => invoke<AutoState>("auto_start", { request }),
  autoCancel: () => invoke("auto_cancel"),
  autoApply: (candidate: Pick<CandidateResult, "engine" | "name" | "template" | "percent">) =>
    invoke<Strategy>("auto_apply", { candidate }),
  getHistory: () => invoke("get_history"),
  readLog: (name: string, lines: number) => invoke<string[]>("read_log", { name, lines }),
  logInfo: (name: string) => invoke<LogInfo>("log_info", { name }),
  clearLog: (name: string) => invoke("clear_log", { name }),
  isElevated: () => invoke<boolean>("is_elevated"),
  relaunchElevated: () => invoke("relaunch_elevated"),
  openUrl: (url: string) => invoke("open_url", { url }),
  appUpdateCheck: () => invoke<AppUpdateInfo | null>("app_update_check"),
  appUpdateInstall: () => invoke("app_update_install"),
  openDataDir: () => invoke("open_data_dir"),
  openLogsDir: () => invoke("open_logs_dir"),
  setAutostart: (enabled: boolean) => invoke("set_autostart", { enabled }),
  deleteStrategy: (id: string) => invoke("delete_strategy", { id }),
};

export type ProgressEvent = Progress;
