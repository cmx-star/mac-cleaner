//! 移到废纸篓。
//!
//! 删除走系统 API `NSFileManager.trashItemAtURL`（经 `osascript` 调 JXA），
//! 而不是自己 `mv` 到 `~/.Trash`：前者会自动处理重名冲突、跨卷移动，
//! 并在废纸篓注册「放回原处」信息，用户能从访达原样恢复。
//!
//! 安全边界全部集中在本模块：
//! 1. 路径必须先通过 `safety::normalize` 与 `safety::classify`，
//!    系统路径、白名单路径一律拒绝。
//! 2. 拒绝根目录、家目录本身、以及过短的路径，避免误删整棵目录树。
//! 3. 一次调用只处理调用方显式列出的路径，不做递归猜测。

use crate::safety;
use serde::Serialize;
use std::path::Path;

/// 单个路径的处理结果。
#[derive(Debug, Clone, Serialize)]
pub struct TrashOutcome {
    pub path: String,
    pub ok: bool,
    /// 失败原因，成功时为 None。
    pub error: Option<String>,
}

/// 判定一条路径是否允许移入废纸篓。
///
/// 返回 Err 时携带面向用户的原因。这些检查是删除的最后一道闸门，
/// 调用方（命令层）必须先过一遍，本模块内部再独立校验一次。
pub fn check_deletable(raw: &str) -> Result<String, String> {
    let normalized = safety::normalize(raw)
        .ok_or_else(|| format!("路径不合法：{raw}"))?;

    // 根目录与家目录本身绝不删除：这两条一旦漏过，后果是整盘或整个用户目录。
    if normalized == "/" {
        return Err("拒绝删除根目录".to_string());
    }
    if let Some(home) = safety::home_dir() {
        if normalized == home {
            return Err("拒绝删除用户主目录".to_string());
        }
    }

    // 顶层目录（如 /Users、/Applications）不允许作为删除目标：
    // 它们的子项才是应用与数据，直接删顶层等于删掉一整类内容。
    let depth = normalized.split('/').filter(|s| !s.is_empty()).count();
    if depth < 2 {
        return Err(format!("拒绝删除顶层目录：{normalized}"));
    }

    // 复用只读扫描的同一套保护规则，保证「扫描时标红」与「删除时拒绝」一致。
    let whitelist = safety::load_whitelist();
    let protection = safety::classify(Path::new(&normalized), &whitelist);
    if protection.is_protected() {
        let reason = protection.reason().unwrap_or("受保护路径");
        return Err(format!("{normalized} 受保护：{reason}"));
    }

    Ok(normalized)
}

/// 把一批路径移入废纸篓，返回每个路径的结果。
///
/// 逐条校验并逐条执行：某一条被拒绝或失败不影响其余条目，
/// 界面因此能显示「哪几个没删掉、为什么」。
pub fn move_to_trash(paths: &[String]) -> Vec<TrashOutcome> {
    let mut outcomes = Vec::with_capacity(paths.len());
    let mut accepted: Vec<String> = Vec::new();

    for raw in paths {
        match check_deletable(raw) {
            Ok(path) => {
                outcomes.push(TrashOutcome {
                    path: path.clone(),
                    ok: false, // 先占位，实际结果由批量执行回填
                    error: None,
                });
                accepted.push(path);
            }
            Err(reason) => outcomes.push(TrashOutcome {
                path: raw.clone(),
                ok: false,
                error: Some(reason),
            }),
        }
    }

    if accepted.is_empty() {
        return outcomes;
    }

    let results = trash_batch(&accepted);

    // 用批量执行的结果回填占位项。
    let mut filled: Vec<TrashOutcome> = Vec::with_capacity(outcomes.len());
    for outcome in outcomes {
        if outcome.error.is_some() {
            filled.push(outcome);
            continue;
        }
        match results.iter().find(|r| r.path == outcome.path) {
            Some(r) => filled.push(TrashOutcome {
                path: outcome.path,
                ok: r.ok,
                error: r.error.clone(),
            }),
            None => filled.push(TrashOutcome {
                path: outcome.path,
                ok: false,
                error: Some("未收到执行结果".to_string()),
            }),
        }
    }
    filled
}

/// 把 accepted 里的路径逐个移入废纸篓。
///
/// 使用系统原生 Cocoa API `NSFileManager.trashItemAtURL`（基于 `trash` crate）。
/// 纯 Rust 原生调用，不依赖外部 `osascript` 进程，支持重名处理、跨卷移动与访达「放回原处」。
fn trash_batch(paths: &[String]) -> Vec<TrashOutcome> {
    paths
        .iter()
        .map(|p| {
            let path = Path::new(p);
            // 如果文件已不存在，按成功处理
            if !path.exists() {
                return TrashOutcome {
                    path: p.clone(),
                    ok: true,
                    error: None,
                };
            }

            // 1. 调用系统原生 API
            match trash::delete(path) {
                Ok(()) => TrashOutcome {
                    path: p.clone(),
                    ok: true,
                    error: None,
                },
                Err(e) => {
                    // 2. 尝试解除只读权限后重试
                    if let Ok(meta) = std::fs::symlink_metadata(path) {
                        let mut perms = meta.permissions();
                        if perms.readonly() {
                            perms.set_readonly(false);
                            let _ = std::fs::set_permissions(path, perms);
                            if trash::delete(path).is_ok() {
                                return TrashOutcome {
                                    path: p.clone(),
                                    ok: true,
                                    error: None,
                                };
                            }
                        }
                    }

                    TrashOutcome {
                        path: p.clone(),
                        ok: false,
                        error: Some(format!("移入废纸篓失败：{e}")),
                    }
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_root_and_home() {
        assert!(check_deletable("/").is_err());
        if let Some(home) = safety::home_dir() {
            assert!(check_deletable(&home).is_err(), "不该允许删除家目录本身");
        }
    }

    #[test]
    fn rejects_relative_and_traversing_paths() {
        assert!(check_deletable("relative/path").is_err());
        assert!(check_deletable("/Applications/../System").is_err());
        assert!(check_deletable("").is_err());
    }

    #[test]
    fn rejects_top_level_directories() {
        // /Users 只有一个路径组件，属于顶层目录，必须拒绝。
        assert!(check_deletable("/Users").is_err());
        assert!(check_deletable("/Applications").is_err());
    }

    #[test]
    fn rejects_system_paths() {
        // 复用 safety 的保护规则：系统路径不可删。
        assert!(check_deletable("/System/Library/CoreServices").is_err());
    }

    #[test]
    fn rejects_control_characters() {
        assert!(check_deletable("/Applications/bad\nname.app").is_err());
    }

    #[test]
    fn allows_typical_user_app_data() {
        // 用户目录下的应用数据是卸载的主要目标，必须放行。
        if let Some(home) = safety::home_dir() {
            let candidate = format!("{home}/Library/Caches/com.example.nonexistent");
            // 路径不存在也应通过校验（校验只看规则，不要求存在），
            // 实际删除时由系统 API 报「文件不存在」。
            let result = check_deletable(&candidate);
            assert!(result.is_ok(), "用户缓存目录应允许删除，实际：{result:?}");
        }
    }

    #[test]
    fn empty_input_yields_empty_outcome() {
        assert!(move_to_trash(&[]).is_empty());
    }

    #[test]
    fn rejected_paths_report_reason_without_running() {
        // 被拒绝的条目必须带原因返回，且不触发任何系统调用。
        let outcomes = move_to_trash(&["/".to_string(), "/Users".to_string()]);
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|o| !o.ok));
        assert!(outcomes.iter().all(|o| o.error.is_some()));
    }

    #[test]
    fn trash_real_file_succeeds() {
        if let Some(home) = safety::home_dir() {
            let tmp_dir = format!("{home}/Downloads");
            let tmp_file = format!("{tmp_dir}/talpa_trash_test_{}.txt", std::process::id());
            if std::fs::write(&tmp_file, "talpa test content").is_ok() {
                assert!(Path::new(&tmp_file).exists());
                let outcomes = move_to_trash(&[tmp_file.clone()]);
                assert_eq!(outcomes.len(), 1);
                assert!(outcomes[0].ok, "原生移入废纸篓应成功: {:?}", outcomes[0].error);
                assert!(!Path::new(&tmp_file).exists(), "文件应已不在原路径");
            }
        }
    }
}
