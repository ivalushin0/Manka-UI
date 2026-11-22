export type Engine = "zapret2" | "zapret" | "byedpi";
export type Source = "builtin" | "store" | "user" | "auto" | "catalog" | "external" | "zapretgui";

export interface StrategyOption {
  variable: string;
  title: string;
  default_value: string;
  values: string[];
}

export interface StrategyFlag {
  key: string;
  title: string;
  default: boolean;
}

export interface Strategy {
  id: string;
  engine: Engine;
  name: string;
  source: Source;
  template: string;
  description?: string | null;
  kit_id?: string | null;
  variables: Record<string, string>;
  options: StrategyOption[];
  flags: StrategyFlag[];
  tcp_ports?: string | null;
  udp_ports?: string | null;
  game_filter: boolean;
  tags: string[];
  score?: number | null;
  created_at: number;
}

export interface Settings {
  enabled: boolean;
  engine: Engine;
  active: Record<string, string>;
  option_value: Record<string, number>;
  flag_value: Record<string, boolean>;
  fake_sni: string;
  block_quic: boolean;
  autostart: boolean;
  minimize_to_tray: boolean;
  start_minimized: boolean;
  tgws_enabled: boolean;
  tgws_port: number;
  tgws_secret: string;
  tgws_pool_size: number;
  tgws_buf_kb: number;
  tgws_cloudflare: boolean;
  tgws_dc_ips: string;
  tgws_extra: string;
  targets: string[];
  custom_targets: string;
  autoselect_full: boolean;
  autoselect_include_store: boolean;
  autoselect_requests: number;
  autoselect_timeout: number;
  autoselect_stop_after_perfect: number;
  autoselect_targets: string[];
  last_check: number;
  last_check_rate: number;
  update_interval_hours: number;
  last_update_check: number;
  update_repo: string;
  auto_update_check: boolean;
  app_update_interval_hours: number;
  last_app_update_check: number;
  check_components_on_start: boolean;
  debug_logs: boolean;
  language: string;
  theme: string;
  close_to_tray: boolean;
}

export interface ProcessStatus {
  running: boolean;
  pid?: number | null;
  uptime_sec: number;
  engine?: string | null;
  detail?: string | null;
}

export interface ComponentStatus {
  id: string;
  name: string;
  kind: string;
  installed: boolean;
  version?: string | null;
  latest?: string | null;
  update_available: boolean;
  path?: string | null;
  note?: string | null;
}

export interface SiteResult {
  url: string;
  ok: number;
  total: number;
  avg_ms: number;
  status?: number | null;
  bytes: number;
  error?: string | null;
}

export interface CandidateResult {
  strategy_id: string;
  name: string;
  engine: Engine;
  source: Source;
  template: string;
  percent: number;
  ok: number;
  total: number;
  avg_ms: number;
  sites: SiteResult[];
  start_error?: string | null;
}

export interface AutoState {
  phase: string;
  engines: Engine[];
  current: number;
  total: number;
  current_name: string;
  current_engine?: Engine | null;
  targets: string[];
  baseline: SiteResult[];
  results: CandidateResult[];
  error?: string | null;
}

export interface SavedResult {
  name: string;
  template: string;
  strategy_id: string;
  engine: string;
  percent: number;
  ok: number;
  total: number;
  avg_ms: number;
  sites: SiteResult[];
  start_error?: string | null;
}

export interface HistoryRun {
  id: string;
  time: number;
  engine?: string | null;
  targets: string[];
  baseline_ok: number;
  baseline_total: number;
  tested: number;
  results: SavedResult[];
}

export interface TargetGroup {
  id: string;
  title: string;
  urls: string[];
}

export interface KitInfo {
  id: string;
  name: string;
  version: string;
}

export interface AppData {
  version: string;
  data_dir: string;
  elevated: boolean;
  settings: Settings;
  strategies: Strategy[];
  engine: ProcessStatus;
  proxy: ProcessStatus;
  components: ComponentStatus[];
  target_groups: TargetGroup[];
  kits: KitInfo[];
  history: HistoryRun[];
  log_tail: string[];
}

export interface StoreItem {
  store_id: string;
  category: string;
  kind: string;
  name: string;
  short_name: string;
  developer: string;
  description: string;
  warning?: string | null;
  version_control: string;
  link: string;
  preferred_file?: string | null;
  installed_version?: string | null;
}

export interface Progress {
  id: string;
  stage: string;
  downloaded: number;
  total: number;
  percent: number;
}

export interface StatusDto {
  engine: ProcessStatus;
  proxy: ProcessStatus;
}

export interface PortOwner {
  pid: number;
  name: string;
}

export interface AutoRequest {
  engines: Engine[];
  targets: string[];
  full: boolean;
  include_store: boolean;
  requests: number;
  timeout_sec: number;
  stop_after_perfect: number;
}

export interface TgwsCheck {
  ok: boolean;
  output: string;
}

export interface LogInfo {
  exists: boolean;
  size: number;
  modified: number;
}

export interface AppUpdateInfo {
  current_version: string;
  version: string;
  notes?: string | null;
  date?: string | null;
  download_url: string;
  releases_url: string;
}
