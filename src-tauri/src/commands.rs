//! Tauri 命令层。
//!
//! 每个命令做参数校验与结果组装，业务逻辑在各自的模块里。
//! 只读命令不触碰文件内容，执行类命令全部转发给 Mole CLI。

use crate::{apps, cli, history, scan, status};
use std::sync::Mutex;
use sysinfo::{Disks, Networks, System};
use tauri::State;

/// 复用的采集器，避免每次刷新都重新建立系统快照。
pub struct Collector {
    pub system: Mutex<System>,
}

impl Default for Collector {
    fn default() -> Self {
        Self {
            system: Mutex::new(System::new_all()),
        }
    }
}

/// 扫描指定目录。
#[tauri::command]
pub async fn scan_directory(path: String) -> Result<scan::ScanResult, String> {
    tauri::async_runtime::spawn_blocking(move || scan::scan_directory(&path))
        .await
        .map_err(|e| format!("扫描任务异常：{e}"))?
}

/// 按扩展名统计目录构成。
#[tauri::command]
pub async fn extension_breakdown(path: String) -> Result<std::collections::HashMap<String, u64>, String> {
    tauri::async_runtime::spawn_blocking(move || scan::extension_breakdown(&path))
        .await
        .map_err(|e| format!("统计任务异常：{e}"))?
}

/// 扫描清理候选。
#[tauri::command]
pub async fn cleanup_preview() -> Result<scan::CleanupPreview, String> {
    tauri::async_runtime::spawn_blocking(scan::scan_cleanup_candidates)
        .await
        .map_err(|e| format!("扫描任务异常：{e}"))?
}

/// 采集系统状态。
#[tauri::command]
pub async fn system_status(collector: State<'_, Collector>) -> Result<status::Snapshot, String> {
    // 采样 CPU 使用率需要两次读数之间的间隔。
    {
        let mut sys = collector
            .system
            .lock()
            .map_err(|_| "状态采集器被占用".to_string())?;
        sys.refresh_cpu_usage();
    }

    tokio::time::sleep(std::time::Duration::from_millis(600)).await;

    let mut sys = collector
        .system
        .lock()
        .map_err(|_| "状态采集器被占用".to_string())?;
    sys.refresh_all();

    let disks = Disks::new_with_refreshed_list();
    let networks = Networks::new_with_refreshed_list();

    Ok(status::collect(&mut sys, &disks, &networks))
}

/// 列出已安装应用。
#[tauri::command]
pub async fn list_apps() -> Result<Vec<apps::App>, String> {
    tauri::async_runtime::spawn_blocking(apps::list_apps)
        .await
        .map_err(|e| format!("应用扫描异常：{e}"))
}

/// 查看某个应用散落在系统各处的数据。
#[tauri::command]
pub async fn app_data(bundle_id: String) -> Result<Vec<apps::AppDataPath>, String> {
    tauri::async_runtime::spawn_blocking(move || apps::app_data_size(&bundle_id))
        .await
        .map_err(|e| format!("数据扫描异常：{e}"))
}

/// 读取操作历史。
#[tauri::command]
pub async fn read_history(limit: Option<usize>) -> Result<history::History, String> {
    let limit = limit.unwrap_or(20).clamp(1, 200);
    tauri::async_runtime::spawn_blocking(move || history::read_history(limit))
        .await
        .map_err(|e| format!("读取历史异常：{e}"))?
}

/// 检查 Mole CLI 是否可用。
#[tauri::command]
pub async fn cli_status() -> Result<cli::CliLocation, String> {
    Ok(cli::locate().await)
}

/// 执行清理。
#[tauri::command]
pub async fn run_clean(dry_run: bool) -> Result<cli::CommandOutput, String> {
    cli::clean(dry_run).await
}

/// 卸载应用。
#[tauri::command]
pub async fn run_uninstall(
    app_name: String,
    dry_run: bool,
) -> Result<cli::CommandOutput, String> {
    cli::uninstall(&app_name, dry_run).await
}

/// 执行系统优化。
#[tauri::command]
pub async fn run_optimize(dry_run: bool) -> Result<cli::CommandOutput, String> {
    cli::optimize(dry_run).await
}

/// 读取用户保护名单。
#[tauri::command]
pub async fn read_whitelist() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(crate::safety::load_whitelist)
        .await
        .map_err(|e| format!("读取保护名单异常：{e}"))
}

/// 用户主目录，分析页用它作为输入框的默认路径。
#[tauri::command]
pub fn home_dir() -> Option<String> {
    crate::safety::home_dir()
}

/// 读取启动项与后台服务。
#[tauri::command]
pub async fn startup_items() -> Result<Vec<crate::startup::StartupItem>, String> {
    tauri::async_runtime::spawn_blocking(crate::startup::list_startup_items)
        .await
        .map_err(|e| format!("读取启动项异常：{e}"))
}

/// 启用或停用一条启动项。
#[tauri::command]
pub async fn set_startup_item(
    id: String,
    enabled: bool,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::startup::set_startup_item_enabled(&id, enabled)
    })
    .await
    .map_err(|e| format!("切换启动项异常：{e}"))?
}

/// 打开系统设置的「登录项与扩展」面板，用于处理本应用改不了的系统托管项。
#[tauri::command]
pub async fn open_login_items_settings() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(crate::startup::open_login_items_settings)
        .await
        .map_err(|e| format!("打开系统设置异常：{e}"))?
}

