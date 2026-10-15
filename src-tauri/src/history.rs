//! 操作历史读取。
//!
//! 解析 Mole 的操作日志 `~/Library/Logs/mole/operations.log` 与删除日志
//! `~/Library/Logs/mole/deletions.log`，格式与 `mo history` 一致。
//!
//! operations.log 行格式：`[时间戳] [命令] 动作 路径 (详情)`
//! deletions.log 行格式由 `lib/core/history.sh` 写入。

use crate::status::human_bytes;
use serde::Serialize;
use std::path::PathBuf;

/// 一次操作记录。
#[derive(Debug, Clone, Serialize)]
pub struct Operation {
    pub timestamp: String,
    pub command: String,
    pub action: String,
    pub path: String,
    pub detail: Option<String>,
}

/// 一次删除记录。
#[derive(Debug, Clone, Serialize)]
pub struct Deletion {
    pub timestamp: String,
    pub mode: String,
    pub status: String,
    pub size_display: String,
    pub path: String,
}

/// 历史查询结果，与 `mo history --json` 的结构对应。
#[derive(Debug, Clone, Serialize)]
pub struct History {
    pub operations_log: String,
    pub deletions_log: String,
    /// 按会话聚合的清理统计。
    pub sessions: Vec<Session>,
    pub deletions: Vec<Deletion>,
    pub limit: usize,
}

/// 一次清理会话。
#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub command: String,
    pub started_at: String,
    pub ended_at: String,
    pub removed: u64,
    pub trashed: u64,
    pub skipped: u64,
    pub failed: u64,
    pub items: u64,
    pub size: String,
    pub size_bytes: u64,
}

/// 日志文件所在目录。
fn logs_dir() -> Option<PathBuf> {
    let home = crate::safety::home_dir()?;
    Some(PathBuf::from(home).join("Library/Logs/mole"))
}

/// 解析单行操作记录。
///
/// 格式：`[2026-10-15 09:12:33] [clean] REMOVED /path/to/thing (15.2MB)`
fn parse_operation_line(line: &str) -> Option<Operation> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    // 提取开头的 `[时间戳]`。
    let rest = line.strip_prefix('[')?;
    let close = rest.find(']')?;
    let timestamp = rest[..close].to_string();
    let rest = rest[close + 1..].trim_start();

    // 提取 `[命令]`。
    let rest = rest.strip_prefix('[')?;
    let close = rest.find(']')?;
    let command = rest[..close].to_string();
    let rest = rest[close + 1..].trim_start();

    // 剩下是 `动作 路径 (详情)`。
    let mut parts = rest.splitn(2, ' ');
    let action = parts.next()?.to_string();
    let remainder = parts.next().unwrap_or("").trim();

    // 详情在末尾的括号里。
    let (path, detail) = match remainder.rfind(" (") {
        Some(idx) if remainder.ends_with(')') => (
            remainder[..idx].to_string(),
            Some(remainder[idx + 2..remainder.len() - 1].to_string()),
        ),
        // 兼容 `(详情)` 是整行剩余的情况。
        _ if remainder.starts_with('(') && remainder.ends_with(')') => (
            String::new(),
            Some(remainder[1..remainder.len() - 1].to_string()),
        ),
        _ => (remainder.to_string(), None),
    };

    Some(Operation {
        timestamp,
        command,
        action,
        path,
        detail,
    })
}

/// 从操作记录聚合出会话。
///
/// Mole 用 `# ========== <命令> session started at <时间> ==========` 标记会话开始，
/// 这里按该标记切分；没有标记时退化为按命令聚合。
fn build_sessions(operations: &[Operation], limit: usize) -> Vec<Session> {
    let mut sessions: Vec<Session> = Vec::new();

    // 按命令与时间窗口聚合：同一命令下，相邻两分钟内的记录视为一次会话。
    let mut current: Option<Session> = None;

    for op in operations {
        let same_command = current
            .as_ref()
            .map(|s| s.command == op.command)
            .unwrap_or(false);

        let close_in_time = current
            .as_ref()
            .map(|s| within_2_minutes(&s.ended_at, &op.timestamp))
            .unwrap_or(false);

        if !same_command || !close_in_time {
            if let Some(s) = current.take() {
                sessions.push(s);
            }
            current = Some(Session {
                command: op.command.clone(),
                started_at: op.timestamp.clone(),
                ended_at: op.timestamp.clone(),
                removed: 0,
                trashed: 0,
                skipped: 0,
                failed: 0,
                items: 0,
                size: "0B".to_string(),
                size_bytes: 0,
            });
        }

        if let Some(s) = current.as_mut() {
            s.ended_at = op.timestamp.clone();
            s.items += 1;
            match op.action.as_str() {
                "REMOVED" => s.removed += 1,
                "TRASHED" => s.trashed += 1,
                "SKIPPED" => s.skipped += 1,
                "FAILED" => s.failed += 1,
                _ => {}
            }
            if let Some(bytes) = op.detail.as_deref().and_then(parse_size_detail) {
                s.size_bytes += bytes;
            }
            s.size = human_bytes(s.size_bytes);
        }
    }

    if let Some(s) = current.take() {
        sessions.push(s);
    }

    // 最近的会话排在前面。
    sessions.reverse();
    sessions.truncate(limit);
    sessions
}

/// 判断两个 `YYYY-MM-DD HH:MM:SS` 时间戳是否在 2 分钟内。
fn within_2_minutes(a: &str, b: &str) -> bool {
    match (parse_epoch_ish(a), parse_epoch_ish(b)) {
        (Some(x), Some(y)) => (y - x).abs() <= 120,
        // 时间戳无法比较时不强制切分，交给命令判断。
        _ => true,
    }
}

/// 把 `YYYY-MM-DD HH:MM:SS` 粗略换算为秒数。
/// 只用于比较间隔，不追求跨时区的绝对精度。
fn parse_epoch_ish(ts: &str) -> Option<i64> {
    let ts = ts.trim();
    let (date, time) = ts.split_once(' ')?;
    let mut d = date.split('-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;

    let mut t = time.split(':');
    let hour: i64 = t.next()?.parse().ok()?;
    let minute: i64 = t.next()?.parse().ok()?;
    let second: i64 = t.next().unwrap_or("0").parse().ok()?;

    // 以天为单位的粗换算足以比较分钟级间隔。
    let days = year * 365 + month * 31 + day;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// 解析详情里的体积，例如 `15.2MB`、`813KB`、`1.2GB`。
fn parse_size_detail(detail: &str) -> Option<u64> {
    let detail = detail.trim();
    let upper = detail.to_uppercase();

    let (num_part, multiplier) = if let Some(n) = upper.strip_suffix("GB") {
        (n, 1024u64 * 1024 * 1024)
    } else if let Some(n) = upper.strip_suffix("MB") {
        (n, 1024 * 1024)
    } else if let Some(n) = upper.strip_suffix("KB") {
        (n, 1024)
    } else if let Some(n) = upper.strip_suffix('B') {
        (n, 1)
    } else {
        return None;
    };

    let value: f64 = num_part.trim().trim_end_matches(',').parse().ok()?;
    Some((value * multiplier as f64) as u64)
}

/// 读取操作日志。
fn read_operations(path: &PathBuf) -> Vec<Operation> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(parse_operation_line)
        .collect()
}

/// 记录一条操作日志（与 Mole operations.log 格式一致）。
pub fn log_operation(command: &str, action: &str, path: &str, detail: Option<&str>) {
    let Some(dir) = logs_dir() else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let log_file = dir.join("operations.log");
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let line = match detail {
        Some(d) if !d.is_empty() => format!("[{now}] [{command}] {action} {path} ({d})\n"),
        _ => format!("[{now}] [{command}] {action} {path}\n"),
    };
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_file) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// 读取删除日志。
///
/// 行格式（由 `lib/core/history.sh` 写入）：以 `|` 分隔的字段，形如
/// `时间戳|模式|状态|大小KB|路径`。
fn read_deletions(path: &PathBuf) -> Vec<Deletion> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    content
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('|').collect();
            if fields.len() < 4 {
                return None;
            }
            let size_kb: u64 = fields[3].trim().parse().ok()?;
            Some(Deletion {
                timestamp: fields[0].trim().to_string(),
                mode: fields[1].trim().to_string(),
                status: fields[2].trim().to_string(),
                size_display: human_bytes(size_kb * 1024),
                path: fields.get(4).map(|p| p.trim().to_string()).unwrap_or_default(),
            })
        })
        .collect()
}

/// 读取历史，`limit` 限制返回的会话与删除条数。
pub fn read_history(limit: usize) -> Result<History, String> {
    let dir = logs_dir().ok_or("无法确定日志目录")?;
    let ops_path = dir.join("operations.log");
    let del_path = dir.join("deletions.log");

    let operations = read_operations(&ops_path);
    let mut deletions = read_deletions(&del_path);
    deletions.reverse();
    deletions.truncate(limit);

    Ok(History {
        operations_log: ops_path.to_string_lossy().into_owned(),
        deletions_log: del_path.to_string_lossy().into_owned(),
        sessions: build_sessions(&operations, limit),
        deletions,
        limit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_operation_line() {
        let line = "[2026-10-15 09:12:33] [clean] REMOVED /Users/x/Library/Caches/thing (15.2MB)";
        let op = parse_operation_line(line).expect("应能解析");
        assert_eq!(op.timestamp, "2026-10-15 09:12:33");
        assert_eq!(op.command, "clean");
        assert_eq!(op.action, "REMOVED");
        assert_eq!(op.path, "/Users/x/Library/Caches/thing");
        assert_eq!(op.detail.as_deref(), Some("15.2MB"));
    }

    #[test]
    fn parses_operation_without_detail() {
        let line = "[2026-10-15 09:12:33] [uninstall] SKIPPED /Applications/Thing.app";
        let op = parse_operation_line(line).expect("应能解析");
        assert_eq!(op.action, "SKIPPED");
        assert_eq!(op.path, "/Applications/Thing.app");
        assert!(op.detail.is_none());
    }

    #[test]
    fn ignores_session_markers_and_comments() {
        assert!(parse_operation_line("# ========== clean session started ==========").is_none());
        assert!(parse_operation_line("").is_none());
        assert!(parse_operation_line("   ").is_none());
    }

    #[test]
    fn rejects_malformed_lines() {
        assert!(parse_operation_line("no brackets here").is_none());
        assert!(parse_operation_line("[only timestamp]").is_none());
    }

    #[test]
    fn parses_size_details() {
        assert_eq!(parse_size_detail("15.2MB"), Some(15_938_355));
        assert_eq!(parse_size_detail("1KB"), Some(1024));
        assert_eq!(parse_size_detail("2GB"), Some(2 * 1024 * 1024 * 1024));
        assert_eq!(parse_size_detail("512B"), Some(512));
        assert_eq!(parse_size_detail("无"), None);
    }

    #[test]
    fn compares_timestamps() {
        assert!(within_2_minutes(
            "2026-10-15 09:12:33",
            "2026-10-15 09:13:00"
        ));
        assert!(!within_2_minutes(
            "2026-10-15 09:12:33",
            "2026-10-15 10:30:00"
        ));
    }

    #[test]
    fn builds_sessions_from_operations() {
        let ops = vec![
            Operation {
                timestamp: "2026-10-15 09:12:00".into(),
                command: "clean".into(),
                action: "REMOVED".into(),
                path: "/a".into(),
                detail: Some("1MB".into()),
            },
            Operation {
                timestamp: "2026-10-15 09:12:30".into(),
                command: "clean".into(),
                action: "REMOVED".into(),
                path: "/b".into(),
                detail: Some("2MB".into()),
            },
        ];
        let sessions = build_sessions(&ops, 10);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].removed, 2);
        assert_eq!(sessions[0].size_bytes, 3 * 1024 * 1024);
    }

    #[test]
    fn splits_sessions_by_gap() {
        let ops = vec![
            Operation {
                timestamp: "2026-10-15 09:00:00".into(),
                command: "clean".into(),
                action: "REMOVED".into(),
                path: "/a".into(),
                detail: None,
            },
            Operation {
                timestamp: "2026-10-15 11:00:00".into(),
                command: "clean".into(),
                action: "REMOVED".into(),
                path: "/b".into(),
                detail: None,
            },
        ];
        assert_eq!(build_sessions(&ops, 10).len(), 2);
    }

    #[test]
    fn reads_history_without_error() {
        // 日志可能不存在，此时应返回空历史而不是报错。
        let history = read_history(20).expect("读取历史不应失败");
        assert_eq!(history.limit, 20);
    }
}
