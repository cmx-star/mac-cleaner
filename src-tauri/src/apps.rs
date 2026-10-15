//! 已安装应用扫描。
//!
//! 复刻 Mole `bin/uninstall.sh --list` 的只读输出：应用名、bundle id、
//! 路径与体积。体积统计包含应用包本体，与 CLI 行为一致。

use crate::status::human_bytes;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

/// 一个已安装应用。
#[derive(Debug, Clone, Serialize)]
pub struct App {
    pub name: String,
    pub bundle_id: String,
    /// `App` 或 `Homebrew`，与 CLI 的 `source` 字段对应。
    pub source: String,
    pub path: String,
    pub size: u64,
    pub size_display: String,
    /// 是否为系统受保护应用，不可卸载。
    pub protected: bool,
    pub protection_reason: Option<String>,
}

/// 应用搜索目录。
const APP_DIRS: &[&str] = &[
    "/Applications",
    "/System/Applications",
];

/// 系统应用保护名单。这些不可卸载。
/// 对应 Mole `app_protection_data.sh` 的 SYSTEM_CRITICAL_BUNDLES 核心条目。
const PROTECTED_BUNDLE_IDS: &[&str] = &[
    "com.apple.finder",
    "com.apple.dock",
    "com.apple.Safari",
    "com.apple.mail",
    "com.apple.systempreferences",
    "com.apple.SystemSettings",
    "com.apple.Spotlight",
    "com.apple.loginwindow",
    "com.apple.Preview",
    "com.apple.TextEdit",
    "com.apple.Notes",
    "com.apple.iCal",
    "com.apple.AddressBook",
    "com.apple.Photos",
    "com.apple.AppStore",
    "com.apple.Terminal",
    "com.apple.ActivityMonitor",
    "com.apple.DiskUtility",
    "com.apple.KeychainAccess",
    "com.apple.FontBook",
    "com.apple.calculator",
    "com.apple.Dictionary",
];

/// 系统关键路径前缀：位于这些目录下的应用一律保护。
const PROTECTED_PATH_PREFIXES: &[&str] = &[
    "/System/",
    "/Library/Apple/",
    "/usr/",
];

/// 判断应用是否受系统保护。
fn is_protected(bundle_id: &str, path: &str) -> (bool, Option<String>) {
    if PROTECTED_PATH_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
    {
        return (true, Some("系统应用，不可卸载".to_string()));
    }
    if PROTECTED_BUNDLE_IDS.iter().any(|id| bundle_id == *id) {
        return (true, Some("系统应用，不可卸载".to_string()));
    }
    (false, None)
}

/// 计算应用包体积。只统计文件，不跟随符号链接。
fn bundle_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// 从 Info.plist 读取 bundle id。
///
/// 优先使用 `defaults read`，它与 macOS 的 plist 解析行为一致，且能处理
/// 二进制 plist。失败时退回文件名推断，但保持 id 为空而不是伪造。
fn read_bundle_id(app_path: &Path) -> String {
    let info = app_path.join("Contents/Info.plist");
    if !info.exists() {
        return String::new();
    }

    let out = Command::new("defaults")
        .arg("read")
        .arg(info.to_string_lossy().as_ref())
        .arg("CFBundleIdentifier")
        .output();

    match out {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
        _ => String::new(),
    }
}

/// 扫描一个应用目录下的所有 `.app`。
fn scan_dir(dir: &Path, apps: &mut Vec<App>) {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();

        if name.ends_with(".app") {
            let bundle_id = read_bundle_id(&path);
            let size = bundle_size(&path);
            let path_str = path.to_string_lossy().into_owned();
            let (protected, reason) = is_protected(&bundle_id, &path_str);

            apps.push(App {
                name: name.trim_end_matches(".app").to_string(),
                bundle_id,
                source: "App".to_string(),
                path: path_str,
                size,
                size_display: human_bytes(size),
                protected,
                protection_reason: reason,
            });
        } else if dir == Path::new("/Applications") {
            // 支持 /Applications 下的子目录分类（如 Adobe 套件目录）。
            scan_dir(&path, apps);
        }
    }
}

/// 列出已安装应用，按体积从大到小排序。
pub fn list_apps() -> Vec<App> {
    let mut apps: Vec<App> = Vec::new();

    for dir in APP_DIRS {
        scan_dir(Path::new(dir), &mut apps);
    }

    // 同一应用可能被扫描到两次（如符号链接目录），按路径去重。
    apps.sort_by(|a, b| a.path.cmp(&b.path));
    apps.dedup_by(|a, b| a.path == b.path);

    apps.sort_by(|a, b| b.size.cmp(&a.size));
    apps
}

/// 读取用户应用数据目录的占用，用于展示「应用散落在系统各处的文件」。
///
/// 对应文章里那个例子：Claude 本体 781MB，`~/Library/Application Support/claude`
/// 却有 7.67GB。真正占空间的常常不是应用包本身。
pub fn app_data_size(bundle_id: &str) -> Vec<AppDataPath> {
    let Some(home) = crate::safety::home_dir() else {
        return Vec::new();
    };
    if bundle_id.is_empty() {
        return Vec::new();
    }

    let candidates = [
        format!("{home}/Library/Application Support/{bundle_id}"),
        format!("{home}/Library/Caches/{bundle_id}"),
        format!("{home}/Library/Preferences/{bundle_id}.plist"),
        format!("{home}/Library/Logs/{bundle_id}"),
        format!("{home}/Library/Saved Application State/{bundle_id}.savedState"),
        format!("{home}/Library/Containers/{bundle_id}"),
        format!("{home}/Library/Group Containers/{bundle_id}"),
        format!("{home}/Library/WebKit/{bundle_id}"),
        format!("{home}/Library/HTTPStorages/{bundle_id}"),
    ];

    let mut found: Vec<AppDataPath> = Vec::new();
    for path_str in candidates {
        let path = PathBuf::from(&path_str);
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };

        let size = if meta.file_type().is_dir() {
            bundle_size(&path)
        } else {
            meta.len()
        };
        if size == 0 {
            continue;
        }

        let protection = crate::safety::classify(&path, &[]);
        found.push(AppDataPath {
            path: path_str,
            size,
            size_display: human_bytes(size),
            protection_reason: protection.reason().map(str::to_string),
        });
    }

    found.sort_by(|a, b| b.size.cmp(&a.size));
    found
}

/// 应用散落的一个数据路径。
#[derive(Debug, Clone, Serialize)]
pub struct AppDataPath {
    pub path: String,
    pub size: u64,
    pub size_display: String,
    pub protection_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_system_apps_by_bundle_id() {
        let (protected, reason) = is_protected("com.apple.finder", "/Applications/Finder.app");
        assert!(protected);
        assert!(reason.is_some());
    }

    #[test]
    fn protects_apps_in_system_tree() {
        let (protected, _) = is_protected("com.example.thing", "/System/Applications/Thing.app");
        assert!(protected);
    }

    #[test]
    fn allows_third_party_apps() {
        let (protected, reason) = is_protected("com.example.editor", "/Applications/Editor.app");
        assert!(!protected);
        assert!(reason.is_none());
    }

    #[test]
    fn lists_some_apps_on_this_machine() {
        let apps = list_apps();
        // 任何 macOS 都至少有 Safari 或系统应用。
        assert!(!apps.is_empty(), "应至少扫描到一个应用");
        // 排序应由大到小。
        for pair in apps.windows(2) {
            assert!(pair[0].size >= pair[1].size);
        }
    }

    #[test]
    fn app_data_size_handles_empty_bundle_id() {
        assert!(app_data_size("").is_empty());
        assert!(app_data_size("com.definitely.not.installed.xyz").is_empty());
    }
}
