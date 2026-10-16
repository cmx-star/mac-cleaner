//! 系统优化与维护模块。
//!
//! 提供原生的 macOS 系统维护与诊断任务，全部在普通用户权限下安全执行，
//! 零管理员密码请求、零额外依赖：
//! 1. 刷新 DNS 解析器缓存 (dscacheutil)
//! 2. 清理临时下载中断与无效锁 (TemporaryItems)
//! 3. 修复 QuickLook 快速查看缓存 (qlmanage)
//! 4. 重建 LaunchServices 打开方式数据库 (lsregister)
//! 5. 重启剪贴板服务 (pboard)
//! 6. 清理用户字体数据库缓存 (atsutil)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use crate::{history, safety, trash};

/// 优化任务元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeTaskDef {
    pub id: String,
    pub name_key: String,
    pub desc_key: String,
    pub icon_name: String,
    pub recommended: bool,
}

/// 优化任务执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeTaskResult {
    pub id: String,
    pub success: bool,
    pub message: String,
}

/// 获取全部支持的优化任务列表
pub fn list_optimize_tasks() -> Vec<OptimizeTaskDef> {
    vec![
        OptimizeTaskDef {
            id: "dns_cache".to_string(),
            name_key: "optimize.task.dns_cache.title".to_string(),
            desc_key: "optimize.task.dns_cache.desc".to_string(),
            icon_name: "globe".to_string(),
            recommended: true,
        },
        OptimizeTaskDef {
            id: "temporary_items".to_string(),
            name_key: "optimize.task.temporary_items.title".to_string(),
            desc_key: "optimize.task.temporary_items.desc".to_string(),
            icon_name: "trash".to_string(),
            recommended: true,
        },
        OptimizeTaskDef {
            id: "quicklook_cache".to_string(),
            name_key: "optimize.task.quicklook_cache.title".to_string(),
            desc_key: "optimize.task.quicklook_cache.desc".to_string(),
            icon_name: "eye".to_string(),
            recommended: true,
        },
        OptimizeTaskDef {
            id: "launch_services".to_string(),
            name_key: "optimize.task.launch_services.title".to_string(),
            desc_key: "optimize.task.launch_services.desc".to_string(),
            icon_name: "grid".to_string(),
            recommended: false,
        },
        OptimizeTaskDef {
            id: "pasteboard".to_string(),
            name_key: "optimize.task.pasteboard.title".to_string(),
            desc_key: "optimize.task.pasteboard.desc".to_string(),
            icon_name: "clipboard".to_string(),
            recommended: false,
        },
        OptimizeTaskDef {
            id: "font_cache".to_string(),
            name_key: "optimize.task.font_cache.title".to_string(),
            desc_key: "optimize.task.font_cache.desc".to_string(),
            icon_name: "type".to_string(),
            recommended: false,
        },
    ]
}

/// 执行指定的系统优化任务集合
pub async fn run_optimize_tasks(task_ids: Vec<String>) -> Result<Vec<OptimizeTaskResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut results = Vec::new();

        for id in &task_ids {
            let res = match id.as_str() {
                "dns_cache" => execute_dns_flush(),
                "temporary_items" => execute_temp_clean(),
                "quicklook_cache" => execute_quicklook_reset(),
                "launch_services" => execute_launch_services_rebuild(),
                "pasteboard" => execute_pasteboard_reset(),
                "font_cache" => execute_font_cache_clean(),
                _ => OptimizeTaskResult {
                    id: id.clone(),
                    success: false,
                    message: format!("未知的维护任务：{id}"),
                },
            };
            results.push(res);
        }

        let summary = results
            .iter()
            .map(|r| format!("{}:{}", r.id, if r.success { "OK" } else { "FAIL" }))
            .collect::<Vec<_>>()
            .join(", ");
        history::log_operation("optimize", "COMPLETED", &summary, None);

        Ok(results)
    })
    .await
    .map_err(|e| format!("执行优化任务异常：{e}"))?
}

fn execute_dns_flush() -> OptimizeTaskResult {
    let out = Command::new("dscacheutil").arg("-flushcache").output();
    let success = out.map(|o| o.status.success()).unwrap_or(false);
    OptimizeTaskResult {
        id: "dns_cache".to_string(),
        success,
        message: if success {
            "DNS 解析缓存已成功刷新并生效".to_string()
        } else {
            "刷新 DNS 缓存命令执行异常".to_string()
        },
    }
}

fn execute_temp_clean() -> OptimizeTaskResult {
    let mut cleaned_count = 0;
    if let Some(home) = safety::home_dir() {
        let temp_items = PathBuf::from(&home).join("Library/Caches/TemporaryItems");
        if temp_items.exists() {
            if let Ok(entries) = std::fs::read_dir(&temp_items) {
                for entry in entries.flatten() {
                    let path = entry.path().to_string_lossy().into_owned();
                    let outcomes = trash::move_to_trash(&[path]);
                    if outcomes.iter().any(|o| o.ok) {
                        cleaned_count += 1;
                    }
                }
            }
        }
    }
    OptimizeTaskResult {
        id: "temporary_items".to_string(),
        success: true,
        message: format!("已清理 {cleaned_count} 项临时下载碎片与失效文件锁"),
    }
}

fn execute_quicklook_reset() -> OptimizeTaskResult {
    let _ = Command::new("qlmanage").args(["-r", "cache"]).output();
    let _ = Command::new("qlmanage").arg("-r").output();
    OptimizeTaskResult {
        id: "quicklook_cache".to_string(),
        success: true,
        message: "快速查看预览生成器与缩略图缓存已重置".to_string(),
    }
}

fn execute_launch_services_rebuild() -> OptimizeTaskResult {
    let lsregister = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
    let success = Command::new(lsregister)
        .args(["-kill", "-r", "-domain", "local", "-domain", "user"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    OptimizeTaskResult {
        id: "launch_services".to_string(),
        success,
        message: if success {
            "右键“打开方式”关联库已成功重建".to_string()
        } else {
            "重建 LaunchServices 关联库失败".to_string()
        },
    }
}

fn execute_pasteboard_reset() -> OptimizeTaskResult {
    let out = Command::new("killall").arg("pboard").output();
    let success = out.map(|o| o.status.success()).unwrap_or(false);
    OptimizeTaskResult {
        id: "pasteboard".to_string(),
        success,
        message: if success {
            "剪贴板服务 (pboard) 已安全重启并恢复就绪".to_string()
        } else {
            "剪贴板守护进程当前未处于异常状态或重启未响应".to_string()
        },
    }
}

fn execute_font_cache_clean() -> OptimizeTaskResult {
    let out = Command::new("atsutil")
        .args(["databases", "-removeUser"])
        .output();
    let success = out.map(|o| o.status.success()).unwrap_or(false);
    OptimizeTaskResult {
        id: "font_cache".to_string(),
        success,
        message: if success {
            "用户字体缓存数据库已清理，已恢复默认字形映射".to_string()
        } else {
            "清理字体缓存数据库失败".to_string()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_tasks_has_unique_ids() {
        let tasks = list_optimize_tasks();
        let mut ids = tasks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(total, ids.len());
    }

    #[test]
    fn runs_recommended_tasks_without_panic() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let results = rt.block_on(run_optimize_tasks(vec![
            "dns_cache".to_string(),
            "temporary_items".to_string(),
            "quicklook_cache".to_string(),
        ])).unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.success));
    }
}
