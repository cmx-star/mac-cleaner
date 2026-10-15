/** 与 Rust 侧结构一一对应的类型定义。 */

export type Protection =
  | "system"
  | "user_data"
  | "ai_state"
  | "model_cache"
  | "whitelisted"
  | "none";

export type Tier = "regenerable" | "expensive" | "irreplaceable";

export type ScanStatus = "complete" | "partial" | "unavailable";

export interface Entry {
  name: string;
  path: string;
  size: number;
  is_dir: boolean;
  scan_status: ScanStatus;
  age_days: number | null;
  protection: Protection;
  protection_reason: string | null;
  is_build_artifact: boolean;
}

export interface ScanResult {
  path: string;
  scan_status: ScanStatus;
  entries: Entry[];
  large_files: Entry[];
  total_size: number;
  total_files: number;
  truncated: boolean;
}

export interface CleanupItem {
  name: string;
  path: string;
  size: number;
  tier: Tier;
  protection: Protection;
  protection_reason: string | null;
  age_days: number | null;
  is_build_artifact: boolean;
  reviewed_by_mole: boolean;
}

export interface CleanupPreview {
  items: CleanupItem[];
  total_regenerable: number;
  whitelist_count: number;
}

export interface Hardware {
  model: string;
  cpu_model: string;
  total_ram: string;
  disk_size: string;
}

export interface Cpu {
  usage: number;
  per_core: number[];
  core_count: number;
  logical_cpu: number;
  load1: number;
  load5: number;
  load15: number;
}

export interface Memory {
  total: number;
  used: number;
  available: number;
  used_percent: number;
  swap_used: number;
  swap_total: number;
}

export interface Disk {
  mount: string;
  device: string;
  used: number;
  total: number;
  used_percent: number;
  fstype: string;
  external: boolean;
}

export interface NetInterface {
  name: string;
  rx_rate_mbs: number;
  tx_rate_mbs: number;
}

export interface Thermal {
  cpu_temp: number;
  gpu_temp: number;
  fan_speed: number;
  system_power: number;
}

export interface Fan {
  rpm: number;
  target_rpm: number;
  min_rpm: number;
  max_rpm: number;
  forced: boolean;
  index: number;
}

export type FanMode = { auto: null } | { manual: number };

export interface ProcessInfo {
  pid: number;
  name: string;
  memory: number;
  cpu_percent: number;
  /** 可执行文件路径。 */
  exe: string | null;
  /** 所属 .app bundle 路径，取不到为 null。 */
  bundle_path: string | null;
  /** 所属 App 的图标（PNG base64），取不到为 null。 */
  icon_png_base64: string | null;
}

export interface Snapshot {
  host: string;
  platform: string;
  os_version: string;
  uptime_seconds: number;
  uptime: string;
  procs: number;
  hardware: Hardware;
  health_score: number;
  health_score_msg: string;
  cpu: Cpu;
  memory: Memory;
  disks: Disk[];
  disk_io: { read_rate: number; write_rate: number };
  network: NetInterface[];
  thermal: Thermal;
}

export interface App {
  name: string;
  bundle_id: string;
  source: string;
  path: string;
  size: number;
  size_display: string;
  version: string;
  last_used_days_ago: number | null;
  icon_png_base64: string | null;
  protected: boolean;
  protection_reason: string | null;
}

export interface AppDataPath {
  path: string;
  size: number;
  size_display: string;
  protection_reason: string | null;
}

export interface Session {
  command: string;
  started_at: string;
  ended_at: string;
  removed: number;
  trashed: number;
  skipped: number;
  failed: number;
  items: number;
  size: string;
  size_bytes: number;
}

export interface Deletion {
  timestamp: string;
  mode: string;
  status: string;
  size_display: string;
  path: string;
}

export interface History {
  operations_log: string;
  deletions_log: string;
  sessions: Session[];
  deletions: Deletion[];
  limit: number;
}

/** 启动项分类，对应界面上的三个分组。 */
export type StartupItemKind = "login_item" | "background" | "service";

export interface StartupItem {
  id: string;
  name: string;
  kind: StartupItemKind;
  enabled: boolean;
  developer: string | null;
  url: string | null;
  parent: string | null;
  /** 本应用能否切换该项。系统托管的项为 false。 */
  toggleable: boolean;
  /** 不可切换时的原因。 */
  reason: string | null;
  /** 所属 .app bundle 路径，用于取图标。 */
  bundle_path: string | null;
  /** 图标（PNG base64）。 */
  icon_png_base64: string | null;
}

export interface CliLocation {
  found: boolean;
  path: string | null;
  version: string | null;
  install_hint: string | null;
}

export interface CommandOutput {
  success: boolean;
  exit_code: number | null;
  stdout: string;
  stderr: string;
}
