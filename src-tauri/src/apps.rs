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
    /// 应用版本（Info.plist 的 CFBundleShortVersionString）。
    pub version: String,
    /// 最近一次使用时间（距今天数）。None 表示无法判断。
    pub last_used_days_ago: Option<u64>,
    /// 应用图标（PNG base64，无图标时为 None）。
    pub icon_png_base64: Option<String>,
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
    plist_string(app_path, "CFBundleIdentifier")
}

/// 从 Info.plist 读取字符串值。
fn plist_string(app_path: &Path, key: &str) -> String {
    let info = app_path.join("Contents/Info.plist");
    if !info.exists() {
        return String::new();
    }

    let out = Command::new("defaults")
        .arg("read")
        .arg(info.to_string_lossy().as_ref())
        .arg(key)
        .output();

    match out {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
        _ => String::new(),
    }
}

/// 读取应用版本。
fn read_version(app_path: &Path) -> String {
    let v = plist_string(app_path, "CFBundleShortVersionString");
    if v.is_empty() {
        plist_string(app_path, "CFBundleVersion")
    } else {
        v
    }
}

/// 计算应用最近使用时间（距今天数）。
///
/// 用 `mdls` 查 kMDItemLastUsedDate；查不到（从未打开过或 Spotlight 未索引）
/// 返回 None，不伪造「很久没用」。
fn last_used_days(app_path: &Path) -> Option<u64> {
    let out = Command::new("mdls")
        .arg("-raw")
        .arg("-name")
        .arg("kMDItemLastUsedDate")
        .arg(app_path.to_string_lossy().as_ref())
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // 输出形如 `2026-05-12 08:33:41 +0000`，`(null)` 表示未索引。
    let text = text.trim();
    if text.is_empty() || text.contains("(null)") || text == "kMDItemLastUsedDate = (null)" {
        return None;
    }
    let date_str = text.lines().next()?.trim();
    let date_part = date_str.split(' ').next()?;

    // 只解析 YYYY-MM-DD，与今天比较得出天数。
    let mut parts = date_part.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;

    let now_days = now_epoch_days();
    let used_days = year * 365 + month * 31 + day;
    Some((now_days - used_days).max(0) as u64)
}

/// 以「年*365 + 月*31 + 日」的粗略天数表示当前时间。
fn now_epoch_days() -> i64 {
    let out = Command::new("date").arg("+%Y-%m-%d").output();
    let Ok(out) = out else {
        return 0;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.trim().split('-');
    let year: i64 = parts.next().unwrap_or("2026").parse().unwrap_or(2026);
    let month: i64 = parts.next().unwrap_or("1").parse().unwrap_or(1);
    let day: i64 = parts.next().unwrap_or("1").parse().unwrap_or(1);
    year * 365 + month * 31 + day
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
            let version = read_version(&path);
            let last_used = last_used_days(&path);

            apps.push(App {
                name: name.trim_end_matches(".app").to_string(),
                bundle_id,
                source: "App".to_string(),
                path: path_str,
                size,
                size_display: human_bytes(size),
                version,
                last_used_days_ago: last_used,
                icon_png_base64: None,
                protected,
                protection_reason: reason,
            });
        } else if dir == Path::new("/Applications") {
            // 支持 /Applications 下的子目录分类（如 Adobe 套件目录）。
            scan_dir(&path, apps);
        }
    }
}

/// 批量读取应用图标（PNG base64），键为应用路径。
///
/// 走一次 `osascript` 调 `NSWorkspace.iconForFile`，而不是每个应用各起
/// sips + base64 两个进程：90 个应用时前者约 1 秒，后者要几秒到十几秒。
/// NSWorkspace 直接给已解码的图标，因此也覆盖了 Asset Catalog
/// （`Assets.car`）的应用，不再局限于有独立 `.icns` 的旧式应用。
///
/// 任何一步失败都返回空 map，界面按无图标处理，不影响列表展示。
pub fn read_icons_base64(paths: &[String]) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;

    let mut icons: HashMap<String, String> = HashMap::new();
    if paths.is_empty() {
        return icons;
    }

    // 32 逻辑像素：Retina 下 NSImage 会产出 64×64 的位图，列表足够清晰，
    // 单张 PNG 约 6KB，90 个应用合计约 500KB。
    const ICON_SIZE: u32 = 32;

    let script = r#"
ObjC.import('AppKit');
ObjC.import('Foundation');
const args = $.NSProcessInfo.processInfo.arguments;
const size = Number(ObjC.unwrap(args.objectAtIndex(4))) || 32;
const paths = [];
for (let i = 5; i < args.count; i++) paths.push(ObjC.unwrap(args.objectAtIndex(i)));
const results = [];
for (const p of paths) {
  try {
    const icon = $.NSWorkspace.sharedWorkspace.iconForFile($.NSString.stringWithString(p));
    const img = $.NSImage.alloc.initWithSize($.NSMakeSize(size, size));
    img.lockFocus;
    icon.drawInRectFromRectOperationFraction(
      $.NSMakeRect(0, 0, size, size), $.NSZeroRect, $.NSCompositeCopy, 1.0);
    img.unlockFocus;
    const rep = $.NSBitmapImageRep.imageRepWithData(img.TIFFRepresentation);
    const png = rep.representationUsingTypeProperties($.NSPNGFileType, $());
    results.push({ path: p, data: ObjC.unwrap(png.base64EncodedStringWithOptions(0)) });
  } catch (e) {
    results.push({ path: p, data: null });
  }
}
JSON.stringify(results);
"#;

    let mut cmd = Command::new("osascript");
    cmd.arg("-l").arg("JavaScript").arg("-e").arg(script);
    cmd.arg(ICON_SIZE.to_string());
    for path in paths {
        cmd.arg(path);
    }

    let Ok(out) = cmd.output() else {
        return icons;
    };
    if !out.status.success() {
        return icons;
    }

    #[derive(serde::Deserialize)]
    struct IconResult {
        path: String,
        data: Option<String>,
    }

    let Ok(parsed) = serde_json::from_slice::<Vec<IconResult>>(&out.stdout) else {
        return icons;
    };

    for item in parsed {
        if let Some(data) = item.data {
            if !data.is_empty() {
                icons.insert(item.path, data);
            }
        }
    }
    icons
}

/// 列出已安装应用，按体积从大到小排序，并附带图标。
pub fn list_apps() -> Vec<App> {
    let mut apps: Vec<App> = Vec::new();

    for dir in APP_DIRS {
        scan_dir(Path::new(dir), &mut apps);
    }

    // 同一应用可能被扫描到两次（如符号链接目录），按路径去重。
    apps.sort_by(|a, b| a.path.cmp(&b.path));
    apps.dedup_by(|a, b| a.path == b.path);

    apps.sort_by(|a, b| b.size.cmp(&a.size));

    // 图标在所有应用收集完毕后一次性批量取，避免 N 次子进程调用。
    let paths: Vec<String> = apps.iter().map(|a| a.path.clone()).collect();
    let icons = read_icons_base64(&paths);
    for app in apps.iter_mut() {
        app.icon_png_base64 = icons.get(&app.path).cloned();
    }

    apps
}

/// 读取单个应用的图标（PNG base64）。按 bundle_id 查找。
pub fn app_icon(bundle_id: &str) -> Option<String> {
    let apps = list_apps();
    let app = apps.into_iter().find(|a| a.bundle_id == bundle_id)?;
    let icons = read_icons_base64(&[app.path.clone()]);
    icons.get(&app.path).cloned()
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

/// 卸载应用（纯 Rust 实现，不依赖外部 CLI）。
///
/// 1. 查找目标应用（按名称、bundle_id 或 .app 路径精确或忽略大小写匹配）；
/// 2. 校验系统保护规则，拒绝删除系统核心或受保护应用；
/// 3. 若应用正在运行，通知应用退出；
/// 4. 收集应用包本体以及散落在 ~/Library 中的全部关联数据目录；
/// 5. 移入废纸篓（dry_run 为 false 时调用 `trash::move_to_trash`）；
/// 6. 记录操作历史日志；
/// 7. 格式化输出人类可读的统计信息，返回 `CommandOutput`。
pub async fn uninstall_app(
    target: &str,
    dry_run: bool,
) -> Result<crate::cli::CommandOutput, String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("应用名称不能为空".to_string());
    }

    // 快速路径：先直接检查常见路径下的 .app，避免全量读图标
    let mut candidate_paths = vec![PathBuf::from(target)];
    if !target.ends_with(".app") {
        candidate_paths.push(PathBuf::from(format!("/Applications/{target}.app")));
        candidate_paths.push(PathBuf::from(format!("/System/Applications/{target}.app")));
        candidate_paths.push(PathBuf::from(format!("/System/Library/CoreServices/{target}.app")));
        if let Some(home) = crate::safety::home_dir() {
            candidate_paths.push(PathBuf::from(format!("{home}/Applications/{target}.app")));
        }
    }

    let app = if let Some(path) = candidate_paths.into_iter().find(|p| p.exists() && p.is_dir()) {
        let bundle_id = read_bundle_id(&path);
        let size = bundle_size(&path);
        let (protected, reason) = is_protected(&bundle_id, &path.to_string_lossy());
        let version = read_version(&path);
        let last_used = last_used_days(&path);
        Some(App {
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            bundle_id,
            source: "App".to_string(),
            path: path.to_string_lossy().into_owned(),
            size,
            size_display: human_bytes(size),
            version,
            last_used_days_ago: last_used,
            icon_png_base64: None,
            protected,
            protection_reason: reason,
        })
    } else {
        // 回退路径：按 bundle_id 或非标路径从全量应用列表中查找
        let all_apps = list_apps();
        all_apps
            .into_iter()
            .find(|a| {
                a.name.eq_ignore_ascii_case(target)
                    || a.bundle_id.eq_ignore_ascii_case(target)
                    || a.path == target
                    || a.name
                        .trim_end_matches(".app")
                        .eq_ignore_ascii_case(target.trim_end_matches(".app"))
            })
    };

    let app = app.ok_or_else(|| format!("未找到应用：{target}"))?;

    if app.protected {
        let reason = app
            .protection_reason
            .unwrap_or_else(|| "系统受保护应用".to_string());
        return Err(format!("拒绝卸载系统受保护应用「{}」：{}", app.name, reason));
    }

    let mut data_items = app_data_size(&app.bundle_id);

    // 补充：根据应用名在常见数据目录中查找（部分第三方软件使用软件名建立文件夹）
    if let Some(home) = crate::safety::home_dir() {
        let name_candidates = [
            format!("{home}/Library/Application Support/{}", app.name),
            format!("{home}/Library/Caches/{}", app.name),
            format!("{home}/Library/Logs/{}", app.name),
        ];
        for cand in name_candidates {
            if data_items.iter().any(|d| d.path == cand) {
                continue;
            }
            let p = PathBuf::from(&cand);
            if let Ok(meta) = std::fs::symlink_metadata(&p) {
                let size = if meta.file_type().is_dir() {
                    bundle_size(&p)
                } else {
                    meta.len()
                };
                if size > 0 {
                    let protection = crate::safety::classify(&p, &[]);
                    data_items.push(AppDataPath {
                        path: cand,
                        size,
                        size_display: human_bytes(size),
                        protection_reason: protection.reason().map(str::to_string),
                    });
                }
            }
        }
    }

    // 待移入废纸篓的项目清单：app 本体 + 关联散落数据文件
    let mut paths_to_trash: Vec<String> = vec![app.path.clone()];
    for item in &data_items {
        paths_to_trash.push(item.path.clone());
    }

    let total_bytes: u64 = app.size + data_items.iter().map(|d| d.size).sum::<u64>();
    let total_size_str = human_bytes(total_bytes);

    if dry_run {
        let mut stdout = format!(
            "【模拟运行】准备卸载应用：{} ({})\n\n",
            app.name, app.bundle_id
        );
        stdout.push_str(&format!(
            "将移入废纸篓的项目（共 {} 项，预计释放空间 {}）：\n",
            paths_to_trash.len(),
            total_size_str
        ));
        stdout.push_str(&format!("  • {} ({})\n", app.path, app.size_display));
        for item in &data_items {
            stdout.push_str(&format!("  • {} ({})\n", item.path, item.size_display));
        }
        stdout.push_str("\n提示：当前为模拟预览模式，未对系统做任何改动。\n");

        return Ok(crate::cli::CommandOutput {
            success: true,
            exit_code: Some(0),
            stdout,
            stderr: String::new(),
        });
    }

    // 真实卸载时：若应用正在运行，尝试通知应用退出（500ms 超时，不阻塞流程）
    if !app.bundle_id.is_empty() {
        let _ = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            tokio::process::Command::new("osascript")
                .arg("-e")
                .arg(format!("tell application id \"{}\" to quit", app.bundle_id))
                .output(),
        )
        .await;
    }

    // 真实卸载：调用 trash::move_to_trash
    let outcomes = crate::trash::move_to_trash(&paths_to_trash);

    let mut success_count = 0;
    let mut failed_count = 0;
    let mut freed_bytes: u64 = 0;

    let mut stdout = format!("已卸载应用：{} ({})\n\n移入废纸篓项目：\n", app.name, app.bundle_id);
    let mut stderr = String::new();

    for outcome in &outcomes {
        let size_str = if outcome.path == app.path {
            freed_bytes += app.size;
            app.size_display.clone()
        } else if let Some(d) = data_items.iter().find(|d| d.path == outcome.path) {
            freed_bytes += d.size;
            d.size_display.clone()
        } else {
            String::new()
        };

        if outcome.ok {
            success_count += 1;
            stdout.push_str(&format!("  ✓ {} ({})\n", outcome.path, size_str));
            crate::history::log_operation("uninstall", "TRASHED", &outcome.path, Some(&size_str));
        } else {
            failed_count += 1;
            let err = outcome.error.as_deref().unwrap_or("未知原因");
            stdout.push_str(&format!("  ✗ {}（失败：{}）\n", outcome.path, err));
            if !stderr.is_empty() {
                stderr.push('\n');
            }
            stderr.push_str(&format!("{}: {}", outcome.path, err));
            crate::history::log_operation("uninstall", "FAILED", &outcome.path, Some(err));
        }
    }

    stdout.push_str(&format!(
        "\n共移入 {} 个项目（{} 失败），释放空间约 {}。\n",
        success_count,
        failed_count,
        human_bytes(freed_bytes)
    ));
    stdout.push_str("所有被删除的项目已安全送至系统废纸篓，可在访达中随时放回原处。");

    let success = success_count > 0 && failed_count == 0;
    Ok(crate::cli::CommandOutput {
        success,
        exit_code: if success { Some(0) } else { Some(1) },
        stdout,
        stderr,
    })
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
    fn list_apps_attaches_icons() {
        // 列表页要显示应用图标，因此 list_apps 必须回填 icon_png_base64。
        // 机器上无 GUI 会话时 osascript 可能取不到图标，这里只要求
        // 「要么有图标，要么为空」，不允许 panic 或产出非法值。
        let apps = list_apps();
        let with_icon = apps.iter().filter(|a| a.icon_png_base64.is_some()).count();
        if apps.is_empty() {
            return;
        }
        for app in &apps {
            if let Some(data) = &app.icon_png_base64 {
                assert!(!data.is_empty(), "图标 base64 不应为空串");
                // PNG 的 base64 以 iVBORw0KGgo 开头。
                assert!(
                    data.starts_with("iVBORw0KGgo"),
                    "{} 的图标不是 PNG：{}",
                    app.name,
                    &data[..data.len().min(24)]
                );
            }
        }
        // 有 GUI 会话时绝大多数应用都应能取到图标。
        assert!(
            with_icon * 2 >= apps.len(),
            "只有 {with_icon}/{} 个应用取到图标，低于半数说明批量取图标失效",
            apps.len()
        );
    }

    #[test]
    fn app_data_size_handles_empty_bundle_id() {
        assert!(app_data_size("").is_empty());
        assert!(app_data_size("com.definitely.not.installed.xyz").is_empty());
    }

    #[test]
    fn reads_version_from_real_app() {
        // 任意已安装应用都应有版本号字段（可能为空字符串但函数不 panic）。
        let apps = list_apps();
        if let Some(first) = apps.first() {
            let _ = read_version(Path::new(&first.path));
        }
    }

    #[test]
    fn last_used_never_panics() {
        let apps = list_apps();
        if let Some(first) = apps.first() {
            // None（未索引）或非负天数，两者都合法。
            if let Some(days) = last_used_days(Path::new(&first.path)) {
                assert!(days < 365 * 20, "最后使用时间不应超过 20 年：{days}");
            }
        }
    }

    #[tokio::test]
    async fn uninstall_rejects_empty_and_nonexistent() {
        assert!(uninstall_app("", false).await.is_err());
        assert!(uninstall_app("   ", true).await.is_err());
        assert!(uninstall_app("com.definitely.not.exist.fakeapp", true).await.is_err());
    }

    #[tokio::test]
    async fn uninstall_protects_system_apps() {
        // Finder 和 Safari 均受保护，必须拒绝卸载
        let res = uninstall_app("Finder", true).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("受保护"));

        let res = uninstall_app("Safari", true).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn uninstall_dry_run_for_installed_app() {
        let app_path = std::env::temp_dir().join("TalpaMockTest.app");
        let contents_dir = app_path.join("Contents");
        let _ = std::fs::create_dir_all(&contents_dir);
        let plist_path = contents_dir.join("Info.plist");
        let plist_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.talpa.mocktest</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
</dict>
</plist>"#;
        let _ = std::fs::write(&plist_path, plist_content);

        let res = uninstall_app(&app_path.to_string_lossy(), true).await;
        let _ = std::fs::remove_dir_all(&app_path);

        assert!(res.is_ok(), "dry_run 应该成功：{res:?}");
        let out = res.unwrap();
        assert!(out.success);
        assert!(out.stdout.contains("模拟运行") || out.stdout.contains("准备卸载"));
        assert!(out.stdout.contains("TalpaMockTest"));
    }
}
