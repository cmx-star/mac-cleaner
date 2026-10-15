/** Tauri invoke 的薄封装与公共工具函数。 */
import { invoke } from "@tauri-apps/api/core";
import type {
  App,
  AppDataPath,
  CleanupPreview,
  CliLocation,
  ProcessInfo,
  CommandOutput,
  History,
  Protection,
  ScanResult,
  Snapshot,
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

export function cliStatus(): Promise<CliLocation> {
  return invoke("cli_status");
}

export function runClean(dryRun: boolean): Promise<CommandOutput> {
  return invoke("run_clean", { dryRun });
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

export function appIcon(bundleId: string): Promise<string | null> {
  return invoke("app_icon", { bundleId });
}

export function topProcesses(limit?: number): Promise<ProcessInfo[]> {
  return invoke("top_processes", { limit: limit ?? null });
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
