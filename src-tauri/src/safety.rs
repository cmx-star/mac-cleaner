//! 路径安全判断。
//!
//! 这里复刻的是 Mole CLI `lib/core/file_ops.sh` 中 `_mole_is_critical_deletion_path`
//! 与 `lib/core/app_protection.sh` 中 `should_protect_path` 的**只读判断部分**。
//!
//! 边界：本模块只用于扫描阶段的归类与展示，不参与任何删除决策。真正的删除
//! 由 `mo clean` / `mo uninstall` 在自己的安全层内完成。因此这里的规则宁可
//! 保守（多判为受保护），也不放宽。

use std::path::{Path, PathBuf};

/// 绝对受保护根，任何情况下都不作为清理候选。
/// 对应 shell 侧的 `exact_roots` 列表。
const PROTECTED_ROOTS: &[&str] = &[
    "/",
    "/Applications",
    "/Library",
    "/Volumes",
    "/Network",
    "/cores",
    "/etc",
    "/home",
    "/net",
    "/tmp",
    "/var",
    "/private",
    "/private/tmp",
    "/private/var",
    "/private/var/tmp",
    "/private/var/folders",
    "/Users",
    "/opt",
    "/opt/homebrew",
];

/// 受保护目录树，其内部任何路径都不可删除。
const PROTECTED_TREES: &[&str] = &[
    "/bin",
    "/dev",
    "/sbin",
    "/usr",
    "/System",
    "/private/etc",
    "/private/var/audit",
    "/private/var/db",
    "/private/var/root",
    "/Library/Apple",
    "/Library/Extensions",
    "/Library/Keychains",
    "/Applications/Finder.app",
    "/Applications/Safari.app",
];

/// 明确的系统级危险路径前缀，对应 shell 侧 case 分支里的字面量。
const DENY_PREFIXES: &[&str] = &[
    "/bin",
    "/dev",
    "/sbin",
    "/System",
    "/Library/Apple",
    "/Library/Extensions",
    "/Library/Keychains",
    "/etc",
    "/private/etc",
    "/var/db",
    "/var/audit",
    "/var/root",
    "/private/var/db",
    "/private/var/audit",
    "/private/var/root",
];

/// 用户态受保护路径片段。这些路径即使名字像缓存也不得作为清理候选，
/// 对应 shell 侧 `should_protect_path` 与 `should_protect_data` 的核心条目。
const PROTECTED_PATH_FRAGMENTS: &[&str] = &[
    // 账号、凭据、邮件、日历、联系人
    "/Library/Accounts",
    "/Library/Keychains",
    "/Library/Mail",
    "/Library/Calendars",
    "/Library/Contacts",
    "/Library/Mobile Documents",
    // 系统 UI 关键缓存
    "com.apple.systempreferences.cache",
    "com.apple.Settings.cache",
    "com.apple.controlcenter.cache",
    "com.apple.finder.cache",
    "com.apple.dock.cache",
    // 音视频子系统缓存
    "com.apple.coreaudio",
    "coreaudiod",
    // 桌面壁纸与航拍屏保素材
    "/Library/Application Support/com.apple.idleassetsd",
    "/Library/Application Support/com.apple.wallpaper",
    // 音频插件
    "/Library/Audio/Plug-Ins",
    "/Library/Application Support/iZotope",
    "/Library/Application Support/LaserSoft Imaging",
    // Mole 自身日志
    "/Library/Logs/mole",
];

/// AI 工具的会话与记忆目录。这些记录无法重建，比代码本身更珍贵。
/// 对应文章与 `should_protect_path` 中 Codex / Claude / Grok 的保护条目。
const AI_STATE_FRAGMENTS: &[&str] = &[
    "/.codex/sessions",
    "/.codex/auth.json",
    "/.codex/history.jsonl",
    "/.codex/session_index.jsonl",
    "/.claude/projects",
    "/.claude/memories",
    "/.claude/plans",
    "/.claude/skills",
    "/.grok/sessions",
    "/Library/Application Support/Codex",
];

/// 模型权重与缓存。这些体积大且重建代价高，Mole 不清理，只显示占用。
/// 对应文章中的第三类「模型文件」与 `DEFAULT_WHITELIST_PATTERNS`。
const MODEL_CACHE_FRAGMENTS: &[&str] = &[
    "/.ollama/models",
    "/.cache/huggingface",
    "/Library/Application Support/LM Studio",
    "/.cache/lm-studio",
];

/// 保护原因，用于界面展示「为什么不清理」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Protection {
    /// 系统路径，绝对不可删除。
    System,
    /// 用户数据或凭据。
    UserData,
    /// AI 会话与记忆。
    AiState,
    /// 模型权重。
    ModelCache,
    /// 命中用户白名单。
    Whitelisted,
    /// 未受保护，可作为清理候选。
    None,
}

impl Protection {
    /// 面向界面的中文原因说明。
    pub fn reason(self) -> Option<&'static str> {
        match self {
            Protection::System => Some("系统路径，不可删除"),
            Protection::UserData => Some("用户数据或凭据，不可重建"),
            Protection::AiState => Some("AI 会话记录，删掉就没了"),
            Protection::ModelCache => Some("模型文件，重下代价高"),
            Protection::Whitelisted => Some("在你的保护名单里"),
            Protection::None => None,
        }
    }

    pub fn is_protected(self) -> bool {
        !matches!(self, Protection::None)
    }
}

/// 规范化路径用于比较：去掉末尾斜杠，折叠重复斜杠。
/// 返回 None 表示路径本身不合法（空、非绝对、含 `..` 组件或控制字符）。
pub fn normalize(input: &str) -> Option<String> {
    if input.is_empty() || !input.starts_with('/') {
        return None;
    }
    if input.chars().any(|c| c.is_control()) {
        return None;
    }

    let mut parts: Vec<&str> = Vec::new();
    for part in input.split('/') {
        match part {
            // 拒绝路径穿越：`..` 必须作为独立组件被消除，不能留下来。
            ".." => return None,
            "" | "." => continue,
            other => parts.push(other),
        }
    }

    if parts.is_empty() {
        return Some("/".to_string());
    }
    Some(format!("/{}", parts.join("/")))
}

/// 判断路径是否等于受保护根，或其符号链接目标指向受保护根。
/// 对应 shell 侧的 `_mole_path_is_same_existing_file`。
fn is_same_existing_file(path: &Path, root: &str) -> bool {
    let root_path = Path::new(root);
    let Ok(a) = path.canonicalize() else {
        return false;
    };
    let Ok(b) = root_path.canonicalize() else {
        return false;
    };
    a == b
}

/// 判断路径是否位于受保护目录树内部。
/// 对应 shell 侧的 `_mole_path_is_within_existing_root`。
fn is_within_existing_root(path: &Path, root: &str) -> bool {
    let root_path = Path::new(root);
    // 两侧都做符号链接解析，避免通过链接绕过前缀判断。
    let canonical_root = root_path.canonicalize().unwrap_or_else(|_| root_path.to_path_buf());
    let canonical_path = match path.canonicalize() {
        Ok(p) => p,
        // 路径不存在时退回字面前缀判断，保持保守。
        Err(_) => return path.starts_with(root_path),
    };
    canonical_path.starts_with(&canonical_root) && canonical_path != canonical_root
}

/// 判断一个绝对路径命中的保护类型。
///
/// `whitelist` 是用户自定义保护名单，来自 `~/.config/mole/whitelist`。
pub fn classify(path: &Path, whitelist: &[String]) -> Protection {
    let Some(raw) = path.to_str() else {
        // 非 UTF-8 路径无法可靠判断，按受保护处理。
        return Protection::System;
    };
    let Some(normalized) = normalize(raw) else {
        return Protection::System;
    };

    // 1. 用户白名单优先于一切判断，因为它表达的是用户显式意愿。
    if whitelist.iter().any(|pattern| matches_pattern(&normalized, pattern)) {
        return Protection::Whitelisted;
    }

    // 2. 字面危险前缀。
    if DENY_PREFIXES.iter().any(|prefix| {
        normalized == *prefix || normalized.starts_with(&format!("{prefix}/"))
    }) {
        return Protection::System;
    }

    // 3. 等于受保护根。
    if PROTECTED_ROOTS
        .iter()
        .any(|root| normalized == *root || is_same_existing_file(path, root))
    {
        return Protection::System;
    }

    // 4. 位于受保护目录树内部。
    if PROTECTED_TREES
        .iter()
        .any(|root| is_within_existing_root(path, root))
    {
        return Protection::System;
    }

    // 5. 用户主目录本身不可删，但其子项可删。
    //    对应 shell 侧「恰好 /Users/<name> 一级不可删」的处理。
    if let Some(home) = home_dir() {
        if normalized == home {
            return Protection::System;
        }
    }

    // 6. AI 会话记录。
    if AI_STATE_FRAGMENTS
        .iter()
        .any(|fragment| normalized.contains(fragment))
    {
        return Protection::AiState;
    }

    // 7. 模型缓存。
    if MODEL_CACHE_FRAGMENTS
        .iter()
        .any(|fragment| normalized.contains(fragment))
    {
        return Protection::ModelCache;
    }

    // 8. 用户数据与系统关键缓存。
    if PROTECTED_PATH_FRAGMENTS
        .iter()
        .any(|fragment| normalized.contains(fragment))
    {
        return Protection::UserData;
    }

    Protection::None
}

/// 极简 glob 匹配，支持 `*` 与 `?`。
/// 用于用户白名单，语义与 shell 侧 `is_path_whitelisted` 的 glob 分支一致。
fn matches_pattern(path: &str, pattern: &str) -> bool {
    let pattern = pattern.trim_end_matches('/');
    if pattern.is_empty() {
        return false;
    }

    // 无通配符时，判断相等、是白名单项的子目录、或是白名单项的父目录。
    // 父目录也要保护，否则删父目录会连带删掉白名单项。
    // 对应 shell 侧 `is_path_whitelisted` 的两个方向判断。
    if !pattern.contains('*') && !pattern.contains('?') {
        return path == pattern
            || path.starts_with(&format!("{pattern}/"))
            || pattern.starts_with(&format!("{path}/"));
    }

    glob_match(path.as_bytes(), pattern.as_bytes())
}

/// 逐字符回溯的 glob 匹配。`*` 不跨越分隔符之外的特殊处理，
/// 因此路径比较按整串进行。
fn glob_match(text: &[u8], pattern: &[u8]) -> bool {
    let (mut t, mut p) = (0usize, 0usize);
    let (mut star_p, mut star_t) = (None, 0usize);

    while t < text.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == text[t]) {
            t += 1;
            p += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star_p = Some(p);
            star_t = t;
            p += 1;
        } else if let Some(sp) = star_p {
            p = sp + 1;
            star_t += 1;
            t = star_t;
        } else {
            return false;
        }
    }

    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

/// 用户主目录，优先使用 `$HOME`。
pub fn home_dir() -> Option<String> {
    std::env::var("HOME").ok().filter(|h| h.starts_with('/'))
}

/// 读取用户保护名单，来自 `~/.config/mole/whitelist`。
/// 格式与 Mole 一致：每行一个路径或 glob，`#` 开头为注释。
pub fn load_whitelist() -> Vec<String> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };
    let path = PathBuf::from(&home).join(".config/mole/whitelist");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };

    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| expand_tilde(line, &home))
        .collect()
}

/// 把开头的 `~` 展开为用户主目录，与 shell 侧的 `${var/#\~/$HOME}` 行为一致。
fn expand_tilde(line: &str, home: &str) -> String {
    if line == "~" {
        return home.to_string();
    }
    match line.strip_prefix("~/") {
        Some(rest) => format!("{home}/{rest}"),
        None => line.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Protection {
        classify(Path::new(s), &[])
    }

    #[test]
    fn rejects_malformed_paths() {
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("relative/path"), None);
        assert_eq!(normalize("/tmp/../etc/passwd"), None);
        assert_eq!(normalize("/tmp/a\nb"), None);
    }

    #[test]
    fn normalizes_redundant_separators() {
        assert_eq!(normalize("/tmp//a///b").as_deref(), Some("/tmp/a/b"));
        assert_eq!(normalize("/tmp/a/").as_deref(), Some("/tmp/a"));
        assert_eq!(normalize("/").as_deref(), Some("/"));
    }

    #[test]
    fn protects_system_roots() {
        assert_eq!(p("/"), Protection::System);
        assert_eq!(p("/System"), Protection::System);
        assert_eq!(p("/System/Library/Caches"), Protection::System);
        assert_eq!(p("/usr/local/bin/tool"), Protection::System);
        assert_eq!(p("/bin/ls"), Protection::System);
        assert_eq!(p("/etc/hosts"), Protection::System);
        assert_eq!(p("/var/db/anything"), Protection::System);
        assert_eq!(p("/Library/Keychains"), Protection::System);
    }

    #[test]
    fn allows_cleanable_user_paths() {
        // 可再生的用户缓存不应被保护。
        assert_eq!(p("/Users/someone/Library/Caches/com.example.app"), Protection::None);
        assert_eq!(p("/private/var/folders/ab/cd/T/tmpfile"), Protection::None);
        assert_eq!(p("/tmp/mole-src/build"), Protection::None);
    }

    #[test]
    fn protects_ai_conversation_state() {
        // 这些记录删掉就没了，必须保护。
        assert_eq!(
            p("/Users/someone/.codex/sessions/2026/rollout.jsonl"),
            Protection::AiState
        );
        assert_eq!(
            p("/Users/someone/.claude/projects/-Users-someone-proj"),
            Protection::AiState
        );
        assert_eq!(p("/Users/someone/.grok/sessions/abc"), Protection::AiState);
    }

    #[test]
    fn protects_model_weights() {
        assert_eq!(
            p("/Users/someone/.ollama/models/blobs/sha256-abc"),
            Protection::ModelCache
        );
        assert_eq!(
            p("/Users/someone/.cache/huggingface/hub/models--x"),
            Protection::ModelCache
        );
    }

    #[test]
    fn protects_user_data() {
        assert_eq!(p("/Users/someone/Library/Mail/V10"), Protection::UserData);
        assert_eq!(p("/Users/someone/Library/Keychains/login"), Protection::UserData);
        assert_eq!(
            p("/Users/someone/Library/Mobile Documents/com~apple~CloudDocs"),
            Protection::UserData
        );
    }

    #[test]
    fn whitelist_takes_priority() {
        let wl = vec!["/Users/someone/Library/Caches/keep-me".to_string()];
        assert_eq!(
            classify(Path::new("/Users/someone/Library/Caches/keep-me"), &wl),
            Protection::Whitelisted
        );
    }

    #[test]
    fn whitelist_supports_globs() {
        let wl = vec!["/Users/someone/Library/Caches/JetBrains*".to_string()];
        assert_eq!(
            classify(Path::new("/Users/someone/Library/Caches/JetBrainsGoLand2025.1"), &wl),
            Protection::Whitelisted
        );
        assert_eq!(
            classify(Path::new("/Users/someone/Library/Caches/Other"), &wl),
            Protection::None
        );
    }

    #[test]
    fn whitelist_protects_parent_of_entry() {
        // 白名单项的所有父目录都要保护，否则删父目录会连带删掉白名单项。
        let wl = vec!["/Users/someone/Library/Caches/a/b".to_string()];
        assert_eq!(
            classify(Path::new("/Users/someone/Library/Caches/a"), &wl),
            Protection::Whitelisted
        );
    }

    #[test]
    fn glob_matches_correctly() {
        assert!(glob_match(b"abc", b"a*"));
        assert!(glob_match(b"abc", b"*c"));
        assert!(glob_match(b"abc", b"a?c"));
        assert!(glob_match(b"abc", b"*"));
        assert!(glob_match(b"abc", b"abc"));
        assert!(!glob_match(b"abc", b"abd"));
        assert!(!glob_match(b"abc", b"a"));
    }

    #[test]
    fn protection_reasons_are_present() {
        assert!(Protection::System.reason().is_some());
        assert!(Protection::AiState.reason().is_some());
        assert!(Protection::None.reason().is_none());
        assert!(Protection::System.is_protected());
        assert!(!Protection::None.is_protected());
    }
}
