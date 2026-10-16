/** Tauri invoke 的薄封装与公共工具函数。 */
import { invoke } from "@tauri-apps/api/core";
import type {
  App,
  AppDataPath,
  CleanupPreview,
  CliLocation,
  CommandOutput,
  Fan,
  FanMode,
  History,
  ProcessDetail,
  ProcessInfo,
  Protection,
  ScanResult,
  Snapshot,
  StartupItem,
} from "./types";

export function scanDirectory(path: string): Promise<ScanResult> {
  return invoke("scan_directory", { path });
}

export function cleanupPreview(): Promise<CleanupPreview> {
  return invoke("cleanup_preview");
}

export function systemStatus(): Promise<Snapshot> {
  return invoke("system_status");
}

export function listApps(): Promise<App[]> {
  return invoke("list_apps");
}

export function appData(bundleId: string): Promise<AppDataPath[]> {
  return invoke("app_data", { bundleId });
}

export function readHistory(limit?: number): Promise<History> {
  return invoke("read_history", { limit: limit ?? null });
}

/** 用户主目录，用作分析页输入框的默认值。 */
export function homeDir(): Promise<string | null> {
  return invoke("home_dir");
}

/** 启动项与后台服务列表。 */
export function startupItems(): Promise<StartupItem[]> {
  return invoke("startup_items");
}

/** 启用或停用一条启动项。 */
export function setStartupItem(id: string, enabled: boolean): Promise<void> {
  return invoke("set_startup_item", { id, enabled });
}

/** 打开系统设置的「登录项与扩展」面板。 */
export function openLoginItemsSettings(): Promise<void> {
  return invoke("open_login_items_settings");
}

export function cliStatus(): Promise<CliLocation> {
  return invoke("cli_status");
}

export function runClean(dryRun: boolean = false, paths?: string[] | null): Promise<CommandOutput> {
  return invoke("run_clean", { paths: paths ?? null, dryRun });
}

export function runUninstall(appName: string, dryRun: boolean): Promise<CommandOutput> {
  return invoke("run_uninstall", { appName, dryRun });
}

export function runOptimize(dryRun: boolean): Promise<CommandOutput> {
  return invoke("run_optimize", { dryRun });
}

export function classifyPath(path: string): Promise<{ protection: Protection; reason: string | null }> {
  return invoke("classify_path", { path });
}

export function fanStatus(): Promise<Fan[]> {
  return invoke("fan_status");
}

export function setFan(mode: FanMode): Promise<void> {
  return invoke("set_fan", { mode });
}

export function appIcon(bundleId: string): Promise<string | null> {
  return invoke("app_icon", { bundleId });
}

export function topProcesses(limit?: number): Promise<ProcessInfo[]> {
  return invoke("top_processes", { limit: limit ?? null });
}

export function toggleLowLatency(enable: boolean): Promise<void> {
  return invoke("toggle_low_latency", { enable });
}

export function killProcess(pid: number, force: boolean): Promise<void> {
  return invoke("kill_process", { pid, force });
}

export function revealInFinder(path: string): Promise<void> {
  return invoke("reveal_in_finder", { path });
}

export function processDetail(pid: number): Promise<ProcessDetail> {
  return invoke("process_detail", { pid });
}

/** 人类可读的体积显示。 */
export function humanBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  if (unit === 0) return `${bytes} B`;
  return `${value.toFixed(2)} ${units[unit]}`;
}
