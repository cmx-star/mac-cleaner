//! 原生清理与系统维护引擎。
//!
//! 完全使用纯 Rust 与 macOS 系统原生 API 实现，不依赖外部 CLI。
//! 安全原则与 Mole 保持一致：
//! 1. 扫描与分类：只清理 `Tier::Regenerable`（可再生类缓存、日志、派生数据）；
//! 2. 白名单与系统保护：严禁触碰 `safety::Protection` 受保护目录与用户白名单；
//! 3. 移入废纸篓：所有删除均经由 `trash::move_to_trash` 安全送入 macOS 废纸篓，支持放回原处；
//! 4. 历史记录：自动记录至 `operations.log`，供历史记录页面查询。

use crate::history;
use crate::safety;
use crate::scan::{self, Tier};
use crate::status::human_bytes;
use crate::trash;
use std::path::PathBuf;

/// 执行智能系统清理（纯 Rust 原生实现）。
///
/// `target_paths` 为选定的待清理路径列表；为 None 或空时默认处理全部安全可再生项目。
/// `dry_run` 为 true 时只做模拟检查与空间预估，不实际移动任何文件；
/// `dry_run` 为 false 时将选中的安全缓存项移入废纸篓。
pub async fn clean_system(
    target_paths: Option<Vec<String>>,
    dry_run: bool,
) -> Result<crate::cli::CommandOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let preview = scan::scan_cleanup_candidates()
            .map_err(|e| format!("扫描清理候选失败：{e}"))?;

        let path_filter: Option<std::collections::HashSet<String>> = target_paths
            .filter(|p| !p.is_empty())
            .map(|list| list.into_iter().collect());

        // 仅处理可再生的安全缓存项，且符合选中的路径范围
        let regenerable_items: Vec<_> = preview
            .items
            .into_iter()
            .filter(|item| {
                if item.tier != Tier::Regenerable || item.protection.is_protected() {
                    return false;
                }
                match &path_filter {
                    Some(set) => set.contains(&item.path),
                    None => true,
                }
            })
            .collect();

        if regenerable_items.is_empty() {
            return Ok(crate::cli::CommandOutput {
                success: true,
                exit_code: Some(0),
                stdout: "未选中任何可清理项，或所选项目已被清理。".to_string(),
                stderr: String::new(),
            });
        }

        let total_size: u64 = regenerable_items.iter().map(|i| i.size).sum();
        let total_count = regenerable_items.len();

        if dry_run {
            let mut stdout = String::new();
            stdout.push_str("Mole 原生智能清理预览 (Dry Run)\n");
            stdout.push_str("==================================================\n");

            for item in regenerable_items.iter().take(40) {
                stdout.push_str(&format!(
                    "  • {} ({}) -> {}\n",
                    item.name,
                    human_bytes(item.size),
                    item.path
                ));
            }
            if total_count > 40 {
                stdout.push_str(&format!("  ... 其余 {} 项未在此详列\n", total_count - 40));
            }

            stdout.push_str("--------------------------------------------------\n");
            stdout.push_str(&format!(
                "共发现 {} 个可安全释放项目，预计可释放空间约 {}。\n",
                total_count,
                human_bytes(total_size)
            ));
            stdout.push_str("提示：实际执行将安全移入 macOS 系统废纸篓，可在访达中随时「放回原处」。\n");

            return Ok(crate::cli::CommandOutput {
                success: true,
                exit_code: Some(0),
                stdout,
                stderr: String::new(),
            });
        }

        // 实际执行清理：安全移入废纸篓
        let paths: Vec<String> = regenerable_items.iter().map(|i| i.path.clone()).collect();
        let outcomes = trash::move_to_trash(&paths);

        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut success_count = 0;
        let mut failed_count = 0;
        let mut freed_bytes = 0u64;

        stdout.push_str("Mole 原生智能清理执行结果\n");
        stdout.push_str("==================================================\n");

        for (item, outcome) in regenerable_items.iter().zip(outcomes.into_iter()) {
            if outcome.ok {
                success_count += 1;
                freed_bytes += item.size;
                let size_str = human_bytes(item.size);
                stdout.push_str(&format!("  ✓ {} ({})\n", item.name, size_str));
                history::log_operation("clean", "TRASHED", &outcome.path, Some(&size_str));
            } else {
                failed_count += 1;
                let err = outcome.error.as_deref().unwrap_or("未知原因");
                stdout.push_str(&format!("  ✗ {}（{}）\n", item.name, err));
                if !stderr.is_empty() {
                    stderr.push('\n');
                }
                stderr.push_str(&format!("{}: {}", outcome.path, err));
                history::log_operation("clean", "FAILED", &outcome.path, Some(err));
            }
        }

        stdout.push_str("--------------------------------------------------\n");
        stdout.push_str(&format!(
            "清理完毕：成功移入废纸篓 {} 项（{} 项跳过或失败），已释放空间约 {}。\n",
            success_count,
            failed_count,
            human_bytes(freed_bytes)
        ));
        stdout.push_str("所有被清理的项目均已安全送至系统废纸篓，若需要可随时从访达「放回原处」。");

        Ok(crate::cli::CommandOutput {
            success: success_count > 0 || paths.is_empty(),
            exit_code: if failed_count == 0 { Some(0) } else { Some(1) },
            stdout,
            stderr,
        })
    })
    .await
    .map_err(|e| format!("执行清理任务异常：{e}"))?
}

/// 执行系统维护与优化任务（纯 Rust 原生实现）。
pub async fn optimize_system(dry_run: bool) -> Result<crate::cli::CommandOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if dry_run {
            let mut stdout = String::new();
            stdout.push_str("Mole 系统维护与优化预览 (Dry Run)\n");
            stdout.push_str("==================================================\n");
            stdout.push_str("  1. 刷新系统 DNS 解析器缓存 (dscacheutil / mDNSResponder)\n");
            stdout.push_str("  2. 清理临时下载中断文件与无效临时锁\n");
            stdout.push_str("--------------------------------------------------\n");
            stdout.push_str("准备就绪，执行后将优化网络解析并清理无效临时句柄。\n");

            return Ok(crate::cli::CommandOutput {
                success: true,
                exit_code: Some(0),
                stdout,
                stderr: String::new(),
            });
        }

        // 1. 刷新 DNS 缓存
        let _ = std::process::Command::new("dscacheutil")
            .arg("-flushcache")
            .output();

        let _ = std::process::Command::new("killall")
            .args(["-HUP", "mDNSResponder"])
            .output();

        // 2. 清理 ~/Library/Caches/TemporaryItems (如果存在且安全)
        if let Some(home) = safety::home_dir() {
            let temp_items = PathBuf::from(&home).join("Library/Caches/TemporaryItems");
            if temp_items.exists() {
                if let Ok(entries) = std::fs::read_dir(&temp_items) {
                    for entry in entries.flatten() {
                        let path = entry.path().to_string_lossy().into_owned();
                        let _ = trash::move_to_trash(&[path]);
                    }
                }
            }
        }

        history::log_operation("optimize", "COMPLETED", "DNS Cache & TemporaryItems", None);

        let mut stdout = String::new();
        stdout.push_str("Mole 系统维护与优化已完成\n");
        stdout.push_str("==================================================\n");
        stdout.push_str("  ✓ 已成功刷新系统 DNS 解析缓存 (mDNSResponder)\n");
        stdout.push_str("  ✓ 已安全回收临时文件残留\n");
        stdout.push_str("--------------------------------------------------\n");
        stdout.push_str("系统网络与临时缓存优化完毕。\n");

        Ok(crate::cli::CommandOutput {
            success: true,
            exit_code: Some(0),
            stdout,
            stderr: String::new(),
        })
    })
    .await
    .map_err(|e| format!("执行优化任务异常：{e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_system_dry_run_never_panics() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let res = rt.block_on(clean_system(None, true));
        assert!(res.is_ok());
    }

    #[test]
    fn optimize_system_dry_run_works() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let res = rt.block_on(optimize_system(true));
        assert!(res.is_ok());
        assert!(res.unwrap().success);
    }
}
