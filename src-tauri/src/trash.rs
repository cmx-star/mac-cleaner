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
use std::process::Command;

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

/// 调用一次 osascript，把 accepted 里的路径全部移入废纸篓。
///
/// 一次调用处理全部路径：每条都起一个 osascript 在删几十个文件时会明显变慢。
fn trash_batch(paths: &[String]) -> Vec<TrashOutcome> {
    const SCRIPT: &str = r#"
ObjC.import('AppKit');
ObjC.import('Foundation');

const args = $.NSProcessInfo.processInfo.arguments;
const fm = $.NSFileManager.defaultManager;
const results = [];

for (let i = 4; i < args.count; i++) {
  const p = ObjC.unwrap(args.objectAtIndex(i));
  try {
    const url = $.NSURL.fileURLWithPath($.NSString.stringWithString(p));
    // trashItemAtURL:resultingItemURL:error: 的 error 是 NSError**，
    // JXA 里需要传一个 Ref 容器接收。
    const errRef = Ref();
    // 由系统完成移动与废纸篓注册，重名、跨卷都由它处理，
    // 用户的「放回原处」也能正常工作。
    const ok = fm.trashItemAtURLResultingItemURLError(url, null, errRef);
    if (ok) {
      results.push({ path: p, ok: true });
    } else {
      let msg = 'unknown';
      try {
        const e = errRef[0];
        if (e) {
          msg = ObjC.unwrap(e.localizedDescription) || ObjC.unwrap(e.description) || String(e);
        }
      } catch (inner) {
        msg = String(inner);
      }
      results.push({ path: p, ok: false, error: String(msg) });
    }
  } catch (e) {
    results.push({ path: p, ok: false, error: String(e) });
  }
}

JSON.stringify(results);
"#;

    let mut cmd = Command::new("osascript");
    cmd.arg("-l").arg("JavaScript").arg("-e").arg(SCRIPT);
    // osascript 的参数里，前几个是脚本与语言标志，脚本用 arguments[4] 起读，
    // 因此这里先补两个占位参数，让用户路径落在 index 4 之后。
    cmd.arg("--");
    cmd.arg("placeholder");
    for p in paths {
        cmd.arg(p);
    }

    let Ok(out) = cmd.output() else {
        return paths
            .iter()
            .map(|p| TrashOutcome {
                path: p.clone(),
                ok: false,
                error: Some("无法启动 osascript".to_string()),
            })
            .collect();
    };

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return paths
            .iter()
            .map(|p| TrashOutcome {
                path: p.clone(),
                ok: false,
                error: Some(if stderr.is_empty() {
                    "osascript 执行失败".to_string()
                } else {
                    stderr.clone()
                }),
            })
            .collect();
    }

    #[derive(serde::Deserialize)]
    struct Raw {
        path: String,
        ok: bool,
        #[serde(default)]
        error: Option<String>,
    }

    match serde_json::from_slice::<Vec<Raw>>(&out.stdout) {
        Ok(list) => list
            .into_iter()
            .map(|r| TrashOutcome {
                path: r.path,
                ok: r.ok,
                error: r.error,
            })
            .collect(),
        Err(e) => paths
            .iter()
            .map(|p| TrashOutcome {
                path: p.clone(),
                ok: false,
                error: Some(format!("解析结果失败：{e}")),
            })
            .collect(),
    }
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
}
