//! Tauri 命令层。
//!
//! 每个命令做参数校验与结果组装，业务逻辑在各自的模块里。
//! 卸载、状态、扫描等核心功能已完全使用 Rust 实现，不依赖外部 CLI。

use crate::{apps, clean, cli, history, scan, status};
use std::sync::Mutex;
use sysinfo::{Disks, Networks, System};
use tauri::State;

/// 复用的采集器，避免每次刷新都重新建立系统快照。
pub struct Collector {
    pub system: Mutex<System>,
    pub networks: Mutex<Networks>,
    pub last_net_refresh: Mutex<Option<std::time::Instant>>,
}

impl Default for Collector {
    fn default() -> Self {
        Self {
            system: Mutex::new(System::new_all()),
            networks: Mutex::new(Networks::new_with_refreshed_list()),
            last_net_refresh: Mutex::new(None),
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
    let now = std::time::Instant::now();

    // 首次采样或无历史采样点时，在 sleep 之前先刷新一次 networks，保证首次就有基准数据
    let need_pre_refresh = {
        let guard = collector
            .last_net_refresh
            .lock()
            .map_err(|_| "采集器锁被占用".to_string())?;
        guard.is_none()
    };

    if need_pre_refresh {
        let mut nets = collector
            .networks
            .lock()
            .map_err(|_| "网络采集器锁被占用".to_string())?;
        nets.refresh(true);
        let mut guard = collector
            .last_net_refresh
            .lock()
            .map_err(|_| "采集器锁被占用".to_string())?;
        *guard = Some(now);
    }

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

    let mut nets = collector
        .networks
        .lock()
        .map_err(|_| "网络采集器锁被占用".to_string())?;

    let after_sleep = std::time::Instant::now();
    let elapsed = {
        let mut guard = collector
            .last_net_refresh
            .lock()
            .map_err(|_| "采集器锁被占用".to_string())?;
        let prev = guard.unwrap_or(now);
        *guard = Some(after_sleep);
        after_sleep.duration_since(prev).as_secs_f64()
    };

    nets.refresh(true);

    Ok(status::collect(&mut sys, &disks, &nets, elapsed))
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

/// 引擎运行状态（纯 Rust 原生实现，不强求外部 CLI）。
#[tauri::command]
pub async fn cli_status() -> Result<cli::CliLocation, String> {
    let loc = cli::locate().await;
    if loc.found {
        Ok(loc)
    } else {
        Ok(cli::CliLocation {
            found: true,
            path: Some("原生 Rust 引擎".to_string()),
            version: Some(format!("原生 v{}", env!("CARGO_PKG_VERSION"))),
            install_hint: None,
        })
    }
}

/// 执行清理（纯 Rust 原生实现，不依赖外部 CLI）。
#[tauri::command]
pub async fn run_clean(paths: Option<Vec<String>>, dry_run: bool) -> Result<cli::CommandOutput, String> {
    clean::clean_system(paths, dry_run).await
}

/// 卸载应用（纯 Rust 实现，不依赖 Mole CLI）。
#[tauri::command]
pub async fn run_uninstall(
    app_name: String,
    dry_run: bool,
) -> Result<cli::CommandOutput, String> {
    apps::uninstall_app(&app_name, dry_run).await
}

/// 执行系统优化（纯 Rust 原生实现，不依赖外部 CLI）。
#[tauri::command]
pub async fn run_optimize(dry_run: bool) -> Result<cli::CommandOutput, String> {
    clean::optimize_system(dry_run).await
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

/// 对单个路径做保护分级，界面展示「为什么不能删」。
#[tauri::command]
pub async fn classify_path(path: String) -> Result<crate::safety::Protection, String> {
    let whitelist = tauri::async_runtime::spawn_blocking(crate::safety::load_whitelist)
        .await
        .map_err(|e| format!("读取保护名单异常：{e}"))?;
    Ok(crate::safety::classify(
        std::path::Path::new(&path),
        &whitelist,
    ))
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

/// 读取单个应用图标（PNG base64）。
#[tauri::command]
pub async fn app_icon(bundle_id: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || crate::apps::app_icon(&bundle_id))
        .await
        .map_err(|e| format!("图标读取异常：{e}"))
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
    /// 所属 App 的图标（PNG base64）。
    pub icon_png_base64: Option<String>,
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
                icon_png_base64: None,
            }
        })
        .collect();

    // 先按内存排，再按 CPU 排，界面各取所需；只保留非系统空闲项。
    procs.retain(|p| p.memory > 0);
    procs.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
    procs.truncate(limit);

    // 截断之后再取图标：只为真正展示的这几行起一次 osascript，
    // 不给全部进程都查图标。
    let bundles: Vec<String> = procs
        .iter()
        .filter_map(|p| p.bundle_path.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let icons = crate::apps::read_icons_base64(&bundles);
    for p in procs.iter_mut() {
        if let Some(bundle) = &p.bundle_path {
            p.icon_png_base64 = icons.get(bundle).cloned();
        }
    }

    Ok(procs)
}

/// 卸载应用：把 App 本体与选中的散落数据移入废纸篓。
///
/// 路径逐条校验（拒系统路径、拒白名单、拒顶层目录），任何一条被拒绝
/// 都不会影响其余条目。不需要 Mole CLI。
#[tauri::command]
pub async fn trash_paths(paths: Vec<String>) -> Result<Vec<crate::trash::TrashOutcome>, String> {
    tauri::async_runtime::spawn_blocking(move || crate::trash::move_to_trash(&paths))
        .await
        .map_err(|e| format!("移入废纸篓异常：{e}"))
}

/// 切换 Wi-Fi 低延迟模式（开关 awdl0 避免信道轮询抖动）。
#[tauri::command]
pub async fn toggle_low_latency(enable: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let arg = if enable { "down" } else { "up" };
        let out = std::process::Command::new("ifconfig")
            .arg("awdl0")
            .arg(arg)
            .output();
        match out {
            Ok(o) if o.status.success() => Ok(()),
            _ => {
                let script = format!("do shell script \"ifconfig awdl0 {}\" with administrator privileges", arg);
                let elevate = std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(&script)
                    .output()
                    .map_err(|e| format!("启动授权窗口失败：{e}"))?;
                if elevate.status.success() {
                    Ok(())
                } else {
                    let err = String::from_utf8_lossy(&elevate.stderr).trim().to_string();
                    if err.contains("User canceled") || err.contains("-128") {
                        Err("已取消管理员授权".to_string())
                    } else {
                        Err(format!("切换低延迟模式失败：{err}"))
                    }
                }
            }
        }
    })
    .await
    .map_err(|e| format!("执行异常：{e}"))?
}

/// 终止指定 PID 的进程。force 为 true 时发送 SIGKILL (-9)，否则发送 SIGTERM (-15)。
#[tauri::command]
pub async fn kill_process(pid: u32, force: bool) -> Result<(), String> {
    if pid <= 1 {
        return Err("禁止终止系统关键进程".to_string());
    }

    tauri::async_runtime::spawn_blocking(move || {
        let signal = if force { "-9" } else { "-15" };
        let output = std::process::Command::new("kill")
            .args(&[signal, &pid.to_string()])
            .output()
            .map_err(|e| format!("执行 kill 失败: {e}"))?;

        if output.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(if err.trim().is_empty() {
                "终止进程失败，可能权限不足或进程已退出".to_string()
            } else {
                err.trim().to_string()
            })
        }
    })
    .await
    .map_err(|e| format!("执行异常: {e}"))?
}

/// 在访达中定位并高亮显示文件或应用包。
#[tauri::command]
pub async fn reveal_in_finder(path: String) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("路径为空".to_string());
    }
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err("目标路径不存在".to_string());
    }

    std::process::Command::new("open")
        .args(&["-R", &path])
        .spawn()
        .map_err(|e| format!("打开访达失败: {e}"))?;

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcessDetail {
    pub pid: u32,
    pub name: String,
    pub icon_png_base64: Option<String>,
    pub bundle_path: Option<String>,
    pub exe: Option<String>,
    pub cwd: Option<String>,
    pub command: String,
    pub user: String,
    pub start_time_human: String,
    pub threads: usize,
    pub open_files: usize,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub listening_ports: Vec<String>,
    pub confidence: String,
    pub basis: String,
    pub launchd_trigger: String,
    pub launchd_service: Option<String>,
    pub parent_tree: Vec<ProcessTreeNode>,
    pub group_summary: ProcessGroupSummary,
    pub related_processes: Vec<RelatedProcess>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcessTreeNode {
    pub name: String,
    pub pid: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcessGroupSummary {
    pub total_procs: usize,
    pub total_cpu: f32,
    pub total_memory: u64,
    pub user: String,
    pub managed_by: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RelatedProcess {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory: u64,
}

fn human_duration(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else if mins > 0 {
        format!("{mins}m")
    } else {
        format!("{secs}s")
    }
}

/// 获取指定进程的全面运行细节与分析（为什么在运行）。
#[tauri::command]
pub async fn process_detail(
    collector: State<'_, Collector>,
    pid: u32,
) -> Result<ProcessDetail, String> {
    let mut sys = collector
        .system
        .lock()
        .map_err(|_| "状态采集器被占用".to_string())?;
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let sysinfo_pid = sysinfo::Pid::from_u32(pid);
    let proc = sys
        .process(sysinfo_pid)
        .ok_or_else(|| "进程未在运行或已退出".to_string())?;

    let name = proc.name().to_string_lossy().into_owned();
    let exe = proc.exe().map(|e| e.to_string_lossy().into_owned());
    let bundle_path = exe.as_deref().and_then(bundle_from_exe);
    let cwd = proc.cwd().map(|c| c.to_string_lossy().into_owned());
    let command = proc
        .cmd()
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");

    let user = std::env::var("USER").unwrap_or_else(|_| "cmx".to_string());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let start_time_human = if proc.start_time() > 0 && now >= proc.start_time() {
        human_duration(now - proc.start_time())
    } else {
        "刚刚".to_string()
    };

    let disk_read_bytes = proc.disk_usage().total_read_bytes;
    let disk_write_bytes = proc.disk_usage().total_written_bytes;

    // 线程数通过 ps -M 获取
    let threads = std::process::Command::new("ps")
        .args(&["-M", &pid.to_string()])
        .output()
        .ok()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.lines().filter(|l| !l.trim().is_empty()).count().saturating_sub(1)
        })
        .unwrap_or(1);

    // 打开文件与监听端口
    let (open_files, listening_ports) = if let Ok(o) = std::process::Command::new("lsof")
        .args(&["-nP", "-p", &pid.to_string()])
        .output()
    {
        let s = String::from_utf8_lossy(&o.stdout);
        let mut count = 0;
        let mut ports = Vec::new();
        for (i, line) in s.lines().enumerate() {
            if i == 0 || line.trim().is_empty() {
                continue;
            }
            count += 1;
            if line.contains("LISTEN") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(endpoint) = parts.get(8) {
                    if !ports.contains(&endpoint.to_string()) {
                        ports.push(endpoint.to_string());
                    }
                }
            }
        }
        (count, ports)
    } else {
        (0, Vec::new())
    };

    // launchd 服务检测
    let (launchd_service, launchd_trigger, managed_by) = if let Ok(o) = std::process::Command::new("launchctl")
        .arg("list")
        .output()
    {
        let s = String::from_utf8_lossy(&o.stdout);
        let pid_str = pid.to_string();
        let mut svc = None;
        for line in s.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.first() == Some(&pid_str.as_str()) {
                if let Some(s_name) = parts.get(2) {
                    let clean = s_name.strip_prefix("application.").unwrap_or(s_name);
                    svc = Some(clean.to_string());
                    break;
                }
            }
        }
        if let Some(s) = svc {
            (Some(s), "launch job demand".to_string(), "由系统后台服务管理".to_string())
        } else {
            (None, "用户交互启动".to_string(), "由用户交互启动".to_string())
        }
    } else {
        (None, "用户交互启动".to_string(), "由用户交互启动".to_string())
    };

    // 溯源父进程链
    let mut parent_tree = Vec::new();
    let mut cur_pid = proc.parent();
    let mut visited = std::collections::HashSet::new();
    visited.insert(sysinfo_pid);
    while let Some(ppid) = cur_pid {
        if !visited.insert(ppid) {
            break;
        }
        if let Some(p) = sys.process(ppid) {
            parent_tree.push(ProcessTreeNode {
                name: p.name().to_string_lossy().into_owned(),
                pid: p.pid().as_u32(),
            });
            cur_pid = p.parent();
        } else {
            break;
        }
    }
    parent_tree.reverse();
    parent_tree.push(ProcessTreeNode {
        name: name.clone(),
        pid,
    });

    // 关联进程与同包子进程
    let target_bundle = bundle_path.as_deref();
    let mut related = Vec::new();
    let mut total_cpu = proc.cpu_usage();
    let mut total_mem = proc.memory();

    for (p_pid, p) in sys.processes() {
        if *p_pid == sysinfo_pid {
            continue;
        }
        let is_child = p.parent() == Some(sysinfo_pid);
        let same_bundle = if let Some(tb) = target_bundle {
            p.exe()
                .and_then(|e| e.to_str())
                .and_then(bundle_from_exe)
                .as_deref()
                == Some(tb)
        } else {
            false
        };

        if is_child || same_bundle {
            total_cpu += p.cpu_usage();
            total_mem += p.memory();
            related.push(RelatedProcess {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                cpu_percent: p.cpu_usage(),
                memory: p.memory(),
            });
        }
    }

    related.sort_by(|a, b| b.memory.cmp(&a.memory));

    // 图标提取
    let icon_png_base64 = if let Some(bp) = &bundle_path {
        let map = crate::apps::read_icons_base64(&[bp.clone()]);
        map.get(bp).cloned()
    } else {
        None
    };

    let confidence = if bundle_path.is_some() {
        "高可信度".to_string()
    } else {
        "中等可信度".to_string()
    };

    let basis = bundle_path
        .clone()
        .or_else(|| exe.clone())
        .unwrap_or_else(|| name.clone());

    let group_summary = ProcessGroupSummary {
        total_procs: related.len() + 1,
        total_cpu,
        total_memory: total_mem,
        user,
        managed_by,
    };

    Ok(ProcessDetail {
        pid,
        name,
        icon_png_base64,
        bundle_path,
        exe,
        cwd,
        command,
        user: group_summary.user.clone(),
        start_time_human,
        threads,
        open_files,
        disk_read_bytes,
        disk_write_bytes,
        listening_ports,
        confidence,
        basis,
        launchd_trigger,
        launchd_service,
        parent_tree,
        group_summary,
        related_processes: related,
    })
}


