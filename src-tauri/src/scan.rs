//! 磁盘扫描。
//!
//! 复刻 Mole `cmd/analyze` 的只读部分：目录逐层占用、大文件、可清理目录识别。
//! 删除不在这里发生，扫描只产出候选与保护原因。

use crate::safety::{self, Protection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

/// 扫描结果的整体完整性，对应 Mole 的 `scan_status`。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Complete,
    Partial,
    Unavailable,
}

/// 一条目录项。
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub scan_status: ScanStatus,
    /// 距今天数，用于界面显示「多久没动过」。
    pub age_days: Option<u64>,
    pub protection: Protection,
    pub protection_reason: Option<String>,
    /// 是否是项目构建产物，删掉重编译即可恢复。
    pub is_build_artifact: bool,
}

/// 一次目录扫描的结果。
#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    pub path: String,
    pub scan_status: ScanStatus,
    pub entries: Vec<Entry>,
    pub large_files: Vec<Entry>,
    pub total_size: u64,
    pub total_files: u64,
    /// 目录项数超出展示上限时为 true。
    pub truncated: bool,
}

/// 项目构建产物目录名。这些删掉后重新编译即可恢复。
/// 与 Mole `cmd/analyze/cleanable.go` 的 `projectDependencyDirs` 对应。
const BUILD_ARTIFACT_DIRS: &[&str] = &[
    // JavaScript / Node
    "node_modules",
    "bower_components",
    ".next",
    ".nuxt",
    ".output",
    ".parcel-cache",
    ".turbo",
    ".vite",
    ".nx",
    ".angular",
    ".svelte-kit",
    ".astro",
    ".docusaurus",
    // 通用构建输出
    "build",
    "dist",
    "target",
    "out",
    "coverage",
    // Python
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".tox",
    "htmlcov",
    ".ipynb_checkpoints",
    // Apple / 移动端
    "DerivedData",
    "Pods",
    "Carthage",
    ".dart_tool",
    // 其他
    ".gradle",
    ".eggs",
];

/// 需要跳过的系统目录，避免扫描遍历整个系统树。
const SKIP_DIRS: &[&str] = &[
    "/System",
    "/Library/Apple",
    "/private/var/db",
    "/dev",
    "/Network",
    "/Volumes",
];

/// 单次扫描的目录项展示上限。
const MAX_ENTRIES: usize = 200;
/// 大文件列表上限。
const MAX_LARGE_FILES: usize = 15;
/// 大文件阈值：500MB。
const LARGE_FILE_THRESHOLD: u64 = 500 * 1024 * 1024;

/// 计算路径占用大小。目录累加子项，文件取元数据长度。
/// 遇到无权限读取的子树时标记为不完整，而不是静默低估。
fn path_size(path: &Path) -> (u64, u64, bool) {
    let mut total = 0u64;
    let mut files = 0u64;
    let mut complete = true;

    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return (0, 0, false);
    };

    if meta.file_type().is_file() {
        return (meta.len(), 1, true);
    }
    if meta.file_type().is_symlink() {
        // 符号链接不计入体积，避免重复计算与跨树跳转。
        return (0, 0, true);
    }
    if !meta.file_type().is_dir() {
        return (0, 0, true);
    }

    for entry in WalkDir::new(path).follow_links(false).into_iter() {
        match entry {
            Ok(e) => {
                if e.file_type().is_file() {
                    if let Ok(m) = e.metadata() {
                        total += m.len();
                        files += 1;
                    }
                }
            }
            Err(_) => {
                // 无权限或中途失败：如实标记为不完整。
                complete = false;
            }
        }
    }

    (total, files, complete)
}

/// 计算路径的最后修改距今的天数。
fn age_days(path: &Path) -> Option<u64> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let elapsed = SystemTime::now().duration_since(modified).ok()?;
    Some(elapsed.as_secs() / 86_400)
}

/// 判断是否为项目构建产物目录名。
fn is_build_artifact_name(name: &str) -> bool {
    BUILD_ARTIFACT_DIRS.contains(&name)
}

/// 判断路径是否应跳过扫描。
fn should_skip(path: &Path) -> bool {
    let Some(s) = path.to_str() else {
        return true;
    };
    SKIP_DIRS.iter().any(|skip| s == *skip || s.starts_with(&format!("{skip}/")))
}

/// 扫描单个目录，返回其直接子项的占用排序结果。
pub fn scan_directory(target: &str) -> Result<ScanResult, String> {
    let normalized = safety::normalize(target)
        .ok_or_else(|| format!("路径不合法：{target}"))?;
    let root = PathBuf::from(&normalized);

    let meta = std::fs::symlink_metadata(&root)
        .map_err(|e| format!("无法访问 {normalized}：{e}"))?;
    if !meta.file_type().is_dir() {
        return Err(format!("{normalized} 不是目录"));
    }

    let whitelist = safety::load_whitelist();
    let mut entries: Vec<Entry> = Vec::new();
    let mut overall_complete = true;

    let read_dir = std::fs::read_dir(&root)
        .map_err(|e| format!("无法读取 {normalized}：{e}"))?;

    for child in read_dir {
        let Ok(child) = child else {
            overall_complete = false;
            continue;
        };
        let child_path = child.path();
        if should_skip(&child_path) {
            continue;
        }

        let name = child
            .file_name()
            .to_string_lossy()
            .into_owned();

        let Ok(child_meta) = std::fs::symlink_metadata(&child_path) else {
            overall_complete = false;
            continue;
        };
        let is_dir = child_meta.file_type().is_dir();

        let (size, _files, complete) = if is_dir {
            path_size(&child_path)
        } else if child_meta.file_type().is_file() {
            (child_meta.len(), 1, true)
        } else {
            (0, 0, true)
        };

        if !complete {
            overall_complete = false;
        }

        let protection = safety::classify(&child_path, &whitelist);

        entries.push(Entry {
            name: name.clone(),
            path: child_path.to_string_lossy().into_owned(),
            size,
            is_dir,
            scan_status: if complete {
                ScanStatus::Complete
            } else {
                ScanStatus::Partial
            },
            age_days: age_days(&child_path),
            protection,
            protection_reason: protection.reason().map(str::to_string),
            is_build_artifact: is_dir && is_build_artifact_name(&name),
        });
    }

    // 按体积从大到小排序，界面直接可用。
    entries.sort_by(|a, b| b.size.cmp(&a.size));

    let total_size: u64 = entries.iter().map(|e| e.size).sum();
    let total_files = entries.len() as u64;
    let truncated = entries.len() > MAX_ENTRIES;
    entries.truncate(MAX_ENTRIES);

    // 大文件单独收集，界面用于提示。
    let large_files = find_large_files(&root, &whitelist);

    Ok(ScanResult {
        path: normalized,
        scan_status: if overall_complete {
            ScanStatus::Complete
        } else {
            ScanStatus::Partial
        },
        entries,
        large_files,
        total_size,
        total_files,
        truncated,
    })
}

/// 在目录内查找超过阈值的大文件，只取前若干项。
/// 默认跳过受保护路径，避免把用户的邮件库或模型当成「垃圾」提示。
fn find_large_files(root: &Path, whitelist: &[String]) -> Vec<Entry> {
    let mut found: Vec<Entry> = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .max_depth(6)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.len() < LARGE_FILE_THRESHOLD {
            continue;
        }
        let path = entry.path();
        let protection = safety::classify(path, whitelist);
        // 受保护文件不作为「可清理的大文件」展示。
        if protection.is_protected() {
            continue;
        }

        found.push(Entry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: path.to_string_lossy().into_owned(),
            size: meta.len(),
            is_dir: false,
            scan_status: ScanStatus::Complete,
            age_days: age_days(path),
            protection,
            protection_reason: None,
            is_build_artifact: false,
        });
    }

    found.sort_by(|a, b| b.size.cmp(&a.size));
    found.truncate(MAX_LARGE_FILES);
    found
}

/// 扫描用户主目录，产出清理预览候选。
///
/// 这里复刻的是 Mole 清理的分类思路：按「可再生 / 重建代价高 / 不可替代」
/// 三档归类，而不是直接给出可删清单。界面据此让用户自己判断。
pub fn scan_cleanup_candidates() -> Result<CleanupPreview, String> {
    let home = safety::home_dir().ok_or("无法确定用户主目录")?;
    let whitelist = safety::load_whitelist();
    let mut items: Vec<CleanupItem> = Vec::new();

    // 用户缓存：~/.cache 与 ~/Library/Caches 下的直接子项。
    let cache_roots = [
        format!("{home}/Library/Caches"),
        format!("{home}/.cache"),
    ];

    for root in &cache_roots {
        let root_path = PathBuf::from(root);
        let Ok(read_dir) = std::fs::read_dir(&root_path) else {
            continue;
        };
        for child in read_dir.filter_map(Result::ok) {
            let path = child.path();
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };

            let protection = safety::classify(&path, &whitelist);
            let tier = match protection {
                // 用户显式保护的项不进入候选。
                Protection::Whitelisted => continue,
                Protection::AiState | Protection::ModelCache => Tier::Irreplaceable,
                Protection::UserData => Tier::Expensive,
                Protection::System => continue,
                Protection::None => Tier::Regenerable,
            };

            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            let (size, _files, _complete) = if meta.file_type().is_dir() {
                path_size(&path)
            } else {
                (meta.len(), 1, true)
            };
            if size == 0 {
                continue;
            }

            items.push(CleanupItem {
                name,
                path: path.to_string_lossy().into_owned(),
                size,
                tier,
                protection,
                protection_reason: protection.reason().map(str::to_string),
                age_days: age_days(&path),
                is_build_artifact: false,
                // 缓存项由 Mole 的清理规则决定是否安全，界面只做预览。
                reviewed_by_mole: true,
            });
        }
    }

    items.sort_by(|a, b| b.size.cmp(&a.size));

    let total: u64 = items
        .iter()
        .filter(|i| i.tier == Tier::Regenerable)
        .map(|i| i.size)
        .sum();

    Ok(CleanupPreview {
        items,
        total_regenerable: total,
        whitelist_count: whitelist.len(),
    })
}

/// 三档安全分类。对应文章里 Mole 对清理目标的分级。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// 可再生：HTTP 缓存、GPU 缓存、编译产物、多数日志。
    Regenerable,
    /// 重建代价高：包管理缓存、本地模型、设备支持文件。
    Expensive,
    /// 不可替代：会话记录、邮件库、照片图库。
    Irreplaceable,
}

impl Tier {
    pub fn label(self) -> &'static str {
        match self {
            Tier::Regenerable => "可再生成",
            Tier::Expensive => "重建代价高",
            Tier::Irreplaceable => "不可替代",
        }
    }
}

/// 清理预览中的一项。
#[derive(Debug, Clone, Serialize)]
pub struct CleanupItem {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub tier: Tier,
    pub protection: Protection,
    pub protection_reason: Option<String>,
    pub age_days: Option<u64>,
    pub is_build_artifact: bool,
    /// 该项是否属于 Mole 清理规则覆盖的范围。
    pub reviewed_by_mole: bool,
}

/// 清理预览结果。
#[derive(Debug, Clone, Serialize)]
pub struct CleanupPreview {
    pub items: Vec<CleanupItem>,
    /// 可再生成项的总体积，界面用于显示「最多可释放」。
    pub total_regenerable: u64,
    pub whitelist_count: usize,
}

/// 按扩展名统计目录构成，界面用于展示占比。
pub fn extension_breakdown(target: &str) -> Result<HashMap<String, u64>, String> {
    let normalized = safety::normalize(target).ok_or_else(|| format!("路径不合法：{target}"))?;
    let root = PathBuf::from(&normalized);

    let mut map: HashMap<String, u64> = HashMap::new();
    for entry in WalkDir::new(&root)
        .follow_links(false)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let ext = entry
            .path()
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_else(|| "无扩展名".to_string());
        *map.entry(ext).or_insert(0) += meta.len();
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_build_artifacts() {
        assert!(is_build_artifact_name("node_modules"));
        assert!(is_build_artifact_name("target"));
        assert!(is_build_artifact_name("DerivedData"));
        assert!(is_build_artifact_name("__pycache__"));
        assert!(!is_build_artifact_name("Documents"));
        assert!(!is_build_artifact_name("src"));
    }

    #[test]
    fn skips_system_trees() {
        assert!(should_skip(Path::new("/System/Library")));
        assert!(should_skip(Path::new("/Volumes/External")));
        assert!(!should_skip(Path::new("/Users/someone/Documents")));
    }

    #[test]
    fn rejects_invalid_scan_targets() {
        assert!(scan_directory("").is_err());
        assert!(scan_directory("relative").is_err());
        assert!(scan_directory("/tmp/../etc").is_err());
    }

    #[test]
    fn rejects_file_as_scan_target() {
        // 扫描目标必须是目录。
        assert!(scan_directory("/etc/hosts").is_err());
    }

    #[test]
    fn scans_tmp_directory() {
        // macOS 的 /tmp 是指向 /private/tmp 的符号链接，先解析再扫描。
        let target = std::fs::canonicalize("/tmp").unwrap();
        let result = scan_directory(target.to_str().unwrap()).expect("应能扫描 /tmp");
        for pair in result.entries.windows(2) {
            assert!(pair[0].size >= pair[1].size);
        }
    }

    #[test]
    fn tier_labels_are_localized() {
        assert_eq!(Tier::Regenerable.label(), "可再生成");
        assert_eq!(Tier::Irreplaceable.label(), "不可替代");
    }
}
