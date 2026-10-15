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

/// 读取全部风扇。
#[tauri::command]
pub async fn fan_status() -> Result<Vec<crate::smc::Fan>, String> {
    tauri::async_runtime::spawn_blocking(crate::smc::fans)
        .await
        .map_err(|e| format!("风扇读取异常：{e}"))
}

/// 设置风扇模式（自动 / 手动转速）。需要已安装 setuid 的 fanctl 助手。
#[tauri::command]
pub async fn set_fan(mode: crate::smc::FanMode) -> Result<(), String> {
    crate::smc::set_fan_mode(mode).await
}

/// 进程列表条目。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    /// 物理内存占用（字节）。
    pub memory: u64,
    /// CPU 使用率（自上次刷新以来的均值）。
    pub cpu_percent: f32,
    /// 进程可执行文件路径，用于反查所属 App。
    pub exe: Option<String>,
    /// 所属 .app bundle 路径（能从 exe 反查到才有）。
    pub bundle_path: Option<String>,
}

/// 从可执行文件路径反查所属 `.app` bundle。
///
/// 形如 `/Applications/Foo.app/Contents/MacOS/Foo` → `/Applications/Foo.app`。
/// 进程不在任何 bundle 内（如系统守护进程、CLI 工具）时返回 None，
/// 界面用首字母占位。
fn bundle_from_exe(exe: &str) -> Option<String> {
    let idx = exe.find(".app/")?;
    let bundle = &exe[..idx + 4];
    // 只接受绝对路径，避免把相对路径当 bundle。
    if bundle.starts_with('/') {
        Some(bundle.to_string())
    } else {
        None
    }
}

/// 列出资源占用最高的进程。
#[tauri::command]
pub async fn top_processes(
    collector: State<'_, Collector>,
    limit: Option<usize>,
) -> Result<Vec<ProcessInfo>, String> {
    let limit = limit.unwrap_or(12).clamp(1, 50);

    let mut sys = collector
        .system
        .lock()
        .map_err(|_| "状态采集器被占用".to_string())?;
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut procs: Vec<ProcessInfo> = sys
        .processes()
        .values()
        .map(|p| {
            let exe = p
                .exe()
                .map(|e| e.to_string_lossy().into_owned());
            let bundle_path = exe.as_deref().and_then(bundle_from_exe);
            ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                memory: p.memory(),
                cpu_percent: p.cpu_usage(),
                exe,
                bundle_path,
            }
        })
        .collect();

    // 先按内存排，再按 CPU 排，界面各取所需；只保留非系统空闲项。
    procs.retain(|p| p.memory > 0);
    procs.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
    procs.truncate(limit);

    Ok(procs)
}
