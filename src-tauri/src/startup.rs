//! 启动项与后台服务。
//!
//! 数据源全面复刻现代 macOS 标准与 Birth / Nektony 实践，**零管理员密码、零授权弹窗**：
//! 1. **现代登录项 (SMAppService / LoginItems)**：扫描 `/Applications` 各 App 内置的
//!    `Contents/Library/LoginItems/*.app`，结合 `launchctl print-disabled gui/<uid>` 确定启停状态。
//! 2. **自启应用 (App-like LaunchAgents)**：扫描 `~/Library/LaunchAgents` 与 `/Library/LaunchAgents`，
//!    识别启动具体应用本体的代理项，作为登录项呈现。
//! 3. **系统服务与后台代理**：扫描 LaunchAgents 与 LaunchDaemons plist 目录。
//! 4. **传统登录项**：`LSSharedFileList` 零权限兜底。
//!
//! **严禁使用 `sfltool`**：`sfltool dumpbtm` 会触发 macOS 管理员权限认证弹窗（system.privilege.admin），
//! 严重打扰用户。所有读取均在普通用户权限下纯文件与 launchctl 查询完成。

use std::ffi::c_void;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 启动项分类。对应界面上的三个分组。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// 登录项：用户登录时启动（包括现代 App 登录项与自启应用）。
    LoginItem,
    /// 允许在后台：后台启动代理 (LaunchAgents)。
    Background,
    /// 系统监控程序：系统级后台守护进程 (LaunchDaemons)。
    Service,
}

/// 一条启动项。
#[derive(Debug, Clone, Serialize)]
pub struct StartupItem {
    /// 稳定标识，用于回传启停操作。取 launchctl service label 或 bundle id。
    pub id: String,
    pub name: String,
    pub kind: ItemKind,
    /// 是否已启用。
    pub enabled: bool,
    /// 开发者名，可能为空。
    pub developer: Option<String>,
    /// 可执行文件或 bundle 路径，可能为空。
    pub url: Option<String>,
    /// 父应用的 bundle id 或名称，用于把子项归属到具体 App。
    pub parent: Option<String>,
    /// 该项是否允许被本应用切换。系统项与需要 root 的守护进程为 false。
    pub toggleable: bool,
    /// 不可切换时给出的原因。
    pub reason: Option<String>,
    /// 所属 .app bundle 的路径，用于取图标。取不到时为空。
    pub bundle_path: Option<String>,
    /// 图标（PNG base64）。
    pub icon_png_base64: Option<String>,
}

/// 读取全部启动项。
pub fn list_startup_items() -> Vec<StartupItem> {
    // 1. 权威读取 launchctl print-disabled 覆盖表
    let disabled = read_disabled_labels();

    // 2. 扫描已安装 App 内置的 LoginItems Helper (现代 SMAppService 登录项)
    let app_helpers = scan_app_login_items(&disabled);

    // 3. 扫描 ~/Library/LaunchAgents, /Library/LaunchAgents, /Library/LaunchDaemons
    let dir_items = scan_launch_dirs();

    // 4. 扫描传统 LSSharedFileList 登录项
    let legacy_login_items = scan_login_items();

    let mut items = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // 优先收录现代 App 登录项
    for item in app_helpers {
        if seen_ids.insert(item.id.clone()) {
            items.push(item);
        }
    }

    // 补充 LaunchAgents 与 LaunchDaemons
    for mut item in dir_items {
        // 如果是 LaunchAgent 且指向 /Applications 下的实际应用，归入登录项 (对应 Birth 的 启动应用)
        if item.kind == ItemKind::Background {
            if let Some(bundle) = &item.bundle_path {
                if bundle.starts_with("/Applications") || bundle.contains("/Applications/") {
                    item.kind = ItemKind::LoginItem;
                }
            }
        }

        // 覆盖 launchctl 权威启停状态
        if let Some(&is_dis) = disabled.get(&item.id) {
            item.enabled = !is_dis;
        }

        let is_dup = items.iter().any(|existing| {
            existing.id == item.id
                || (existing.url.is_some() && existing.url == item.url)
                || (existing.bundle_path.is_some() && existing.bundle_path == item.bundle_path && existing.kind == item.kind)
        });

        if !is_dup && seen_ids.insert(item.id.clone()) {
            items.push(item);
        }
    }

    // 补充传统 LSSharedFileList 登录项
    for item in legacy_login_items {
        let is_dup = items.iter().any(|existing| {
            existing.id == item.id
                || (existing.url.is_some() && existing.url == item.url)
                || (existing.bundle_path.is_some() && existing.bundle_path == item.bundle_path)
        });
        if !is_dup && seen_ids.insert(item.id.clone()) {
            items.push(item);
        }
    }

    // 统一按名称不区分大小写排序
    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // 批量加载应用图标
    let bundles: Vec<String> = items
        .iter()
        .filter_map(|i| i.bundle_path.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    if !bundles.is_empty() {
        let icons = crate::apps::read_icons_base64(&bundles);
        for item in items.iter_mut() {
            if let Some(bundle) = &item.bundle_path {
                item.icon_png_base64 = icons.get(bundle).cloned();
            }
        }
    }

    items
}

/// 扫描已安装应用内置的登录项 Helper（macOS 现代 SMAppService / SMLoginItemSetEnabled 机制）。
///
/// 诸如 1Password、企业微信、Tailscale、iHosts、MacCleaner Pro 等现代 macOS 应用，
/// 均在 `.app/Contents/Library/LoginItems/*.app` 内放置开机自启动 Helper。
/// 结合 `launchctl print-disabled gui/<uid>`，可瞬间确认其是否被系统注册及真实启停状态。
/// 全程纯文件遍历与 launchctl 只读查询，零管理员密码、零授权弹窗、不调用任何 sfltool。
fn scan_app_login_items(disabled_map: &std::collections::HashMap<String, bool>) -> Vec<StartupItem> {
    let mut items = Vec::new();
    let mut search_dirs = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
    ];
    if let Some(home) = crate::safety::home_dir() {
        search_dirs.push(PathBuf::from(format!("{home}/Applications")));
    }

    // 收集所有顶层及一层子目录下的 .app 路径（如 /Applications/MacCleaner 3 Pro/App Cleaner 8.app）
    let mut app_paths = Vec::new();
    for base in search_dirs {
        let Ok(read_dir) = std::fs::read_dir(base) else { continue };
        for entry in read_dir.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("app") {
                app_paths.push(path);
            } else if path.is_dir() {
                if let Ok(sub_read) = std::fs::read_dir(&path) {
                    for sub_entry in sub_read.filter_map(Result::ok) {
                        let sub_path = sub_entry.path();
                        if sub_path.extension().and_then(|e| e.to_str()) == Some("app") {
                            app_paths.push(sub_path);
                        }
                    }
                }
            }
        }
    }

    for app_path in app_paths {
        let login_items_dir = app_path.join("Contents/Library/LoginItems");
        if !login_items_dir.is_dir() {
            continue;
        }

        let parent_bundle = app_path.to_string_lossy().into_owned();
        let parent_name = app_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let Ok(helper_entries) = std::fs::read_dir(&login_items_dir) else { continue };
        for helper_entry in helper_entries.filter_map(Result::ok) {
            let helper_path = helper_entry.path();
            if helper_path.extension().and_then(|e| e.to_str()) != Some("app") {
                continue;
            }

            let helper_plist = helper_path.join("Contents/Info.plist");
            if !helper_plist.exists() {
                continue;
            }

            let (bid, display_name) = match Command::new("plutil")
                .args(["-convert", "json", "-o", "-"])
                .arg(&helper_plist)
                .stdin(std::process::Stdio::null())
                .output()
            {
                Ok(out) if out.status.success() => {
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                        let bid = val.get("CFBundleIdentifier")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let dname = val.get("CFBundleDisplayName")
                            .and_then(|v| v.as_str())
                            .or_else(|| val.get("CFBundleName").and_then(|v| v.as_str()))
                            .map(|s| s.to_string());
                        (bid, dname)
                    } else {
                        (String::new(), None)
                    }
                }
                _ => (String::new(), None),
            };

            let default_name = helper_path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| parent_name.clone());
            let name = display_name.unwrap_or(default_name);

            // 在 launchctl print-disabled 中匹配对应的注册项
            let mut matched_key = None;
            let mut is_disabled = None;

            if !bid.is_empty() {
                if let Some(&dis) = disabled_map.get(&bid) {
                    matched_key = Some(bid.clone());
                    is_disabled = Some(dis);
                } else {
                    for (k, &dis) in disabled_map {
                        if k.ends_with(&format!(".{bid}")) || bid.ends_with(&format!(".{k}")) || k == &bid {
                            matched_key = Some(k.clone());
                            is_disabled = Some(dis);
                            break;
                        }
                    }
                }
            }

            let (final_id, enabled) = if let (Some(k), Some(dis)) = (matched_key, is_disabled) {
                (k, !dis)
            } else if !bid.is_empty() {
                (bid, true)
            } else {
                continue;
            };

            items.push(StartupItem {
                id: final_id,
                name,
                kind: ItemKind::LoginItem,
                enabled,
                developer: None,
                url: Some(helper_path.to_string_lossy().into_owned()),
                parent: Some(parent_name.clone()),
                toggleable: true,
                reason: None,
                bundle_path: Some(parent_bundle.clone()),
                icon_png_base64: None,
            });
        }
    }

    items
}

/// 读取「登录项」列表。
///
/// 走 `LSSharedFileList` 的 `kLSSharedFileListSessionLoginItems`。该 API 自
/// macOS 10.11 起标记废弃，但截至 macOS 15/26 仍是**唯一零授权**读取系统
/// 登录项列表的途径：不弹 TCC 授权框、不需要管理员密码。
/// 用 `System Events`（AppleScript）也能读，但那会触发「自动化」授权。
///
/// 读取失败（API 未来被移除）时返回空列表，不影响 launchd 项的展示。
fn scan_login_items() -> Vec<StartupItem> {
    // CoreServices 里导出的符号；kLSSharedFileListSessionLoginItems 是个
    // 全局 CFStringRef 变量，用 static 引用它的地址。
    extern "C" {
        static kLSSharedFileListSessionLoginItems: *const c_void;
        fn LSSharedFileListCreate(
            allocator: *const c_void,
            list_type: *const c_void,
            options: *const c_void,
        ) -> *const c_void;
        fn LSSharedFileListCopySnapshot(list: *const c_void, seed: *mut u32) -> *const c_void;
        fn LSSharedFileListItemCopyResolvedURL(
            item: *const c_void,
            flags: u32,
            error: *mut *const c_void,
        ) -> *const c_void;
        fn LSSharedFileListItemCopyDisplayName(item: *const c_void) -> *const c_void;

        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, idx: isize) -> *const c_void;
        fn CFURLCopyFileSystemPath(url: *const c_void, style: i64) -> *const c_void;
        fn CFStringGetCString(
            s: *const c_void,
            buf: *mut i8,
            size: isize,
            encoding: u32,
        ) -> u8;
        fn CFRelease(cf: *const c_void);
    }

    /// kCFURLPOSIXPathStyle
    const URL_POSIX_PATH: i64 = 0;
    /// kCFStringEncodingUTF8
    const UTF8: u32 = 0x0800_0100;
    /// 不触发用户交互、不挂载卷：解析网络共享上的项时不能弹窗或卡住。
    const NO_PROMPT_FLAGS: u32 = 0x0000_0002 | 0x0000_0004;

    fn cfstring_to_string(s: *const c_void) -> Option<String> {
        if s.is_null() {
            return None;
        }
        let mut buf = [0i8; 1024];
        let ok = unsafe { CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as isize, UTF8) };
        if ok == 0 {
            return None;
        }
        let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
        Some(cstr.to_string_lossy().into_owned())
    }

    let mut items = Vec::new();

    unsafe {
        let list = LSSharedFileListCreate(
            std::ptr::null(),
            kLSSharedFileListSessionLoginItems,
            std::ptr::null(),
        );
        if list.is_null() {
            return items;
        }

        let mut seed: u32 = 0;
        let snapshot = LSSharedFileListCopySnapshot(list, &mut seed);
        if snapshot.is_null() {
            CFRelease(list);
            return items;
        }

        let count = CFArrayGetCount(snapshot);
        for i in 0..count {
            let raw = CFArrayGetValueAtIndex(snapshot, i);
            if raw.is_null() {
                continue;
            }

            let url = LSSharedFileListItemCopyResolvedURL(raw, NO_PROMPT_FLAGS, std::ptr::null_mut());
            if url.is_null() {
                continue;
            }
            let path_ref = CFURLCopyFileSystemPath(url, URL_POSIX_PATH);
            let path = cfstring_to_string(path_ref);
            if !path_ref.is_null() {
                CFRelease(path_ref);
            }
            CFRelease(url);

            let Some(path) = path else { continue };
            if path.is_empty() {
                continue;
            }

            let name_ref = LSSharedFileListItemCopyDisplayName(raw);
            let display = cfstring_to_string(name_ref).filter(|s| !s.is_empty());
            if !name_ref.is_null() {
                CFRelease(name_ref);
            }

            let bundle_path = if path.ends_with(".app") { Some(path.clone()) } else { None };
            let name = display
                .or_else(|| {
                    bundle_path
                        .as_deref()
                        .and_then(|p| Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()))
                })
                .unwrap_or_else(|| path.clone());

            items.push(StartupItem {
                // 登录项没有 launchd label，用路径作为稳定标识；
                // 开关操作不需要它（走 System Events 按路径删除）。
                id: format!("login-item:{path}"),
                name,
                kind: ItemKind::LoginItem,
                // LSSharedFileList 的条目只要在列表里就是启用的；
                // 被关掉的项会从列表移除，而不是标记为 disabled。
                enabled: true,
                developer: None,
                url: Some(path),
                parent: None,
                // 登录项由 System Events 按路径增删，首次会触发一次
                // 「自动化」授权，之后不再弹框。
                toggleable: true,
                reason: None,
                bundle_path,
                icon_png_base64: None,
            });
        }

        CFRelease(snapshot);
        CFRelease(list);
    }

    items
}

/// 扫描四个 launchd 目录，读 plist 生成启动项。
fn scan_launch_dirs() -> Vec<StartupItem> {
    let mut items = Vec::new();

    // 顺序即优先级：用户目录优先，便于同名项取到用户级配置。
    let mut dirs: Vec<(String, ItemKind)> = Vec::new();
    if let Some(home) = crate::safety::home_dir() {
        dirs.push((format!("{home}/Library/LaunchAgents"), ItemKind::Background));
    }
    dirs.push(("/Library/LaunchAgents".to_string(), ItemKind::Service));
    dirs.push(("/Library/LaunchDaemons".to_string(), ItemKind::Service));

    for (dir, kind) in dirs {
        let Ok(read_dir) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read_dir.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("plist") {
                continue;
            }
            if let Some(item) = parse_launch_plist(&path, kind) {
                items.push(item);
            }
        }
    }

    items
}

/// 用 `plutil -convert json` 把 plist 读成 JSON，避免手写 plist 解析。
///
/// `plutil` 是系统自带工具，转换到 stdout 不落地任何文件。
fn parse_launch_plist(path: &Path, kind: ItemKind) -> Option<StartupItem> {
    let out = Command::new("plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(path)
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }

    let value: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let obj = value.as_object()?;

    let label = obj.get("Label")?.as_str()?.to_string();

    // 可执行路径：取 ProgramArguments[0]，或用 Program 字段。
    let program = obj
        .get("ProgramArguments")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| obj.get("Program").and_then(|v| v.as_str()).map(str::to_string));

    // `ProgramArguments` 里可能带 `/Applications/Foo.app` 这种参数
    // （如 `/usr/bin/open -a /Applications/ToDesk.app`），一并用于反查图标。
    let bundle_path = program
        .as_deref()
        .and_then(bundle_from_program)
        .or_else(|| {
            obj.get("ProgramArguments")
                .and_then(|v| v.as_array())
                .and_then(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .filter_map(bundle_from_program)
                        .next()
                })
        });

    // plist 里的 Disabled=1 表示停用；缺省视为启用。
    let enabled = obj
        .get("Disabled")
        .and_then(|v| v.as_i64())
        .map(|v| v == 0)
        .unwrap_or(true);

    // 界面展示名：优先用 bundle 文件名，没有就用 Label。
    let name = bundle_path
        .as_deref()
        .and_then(|p| Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| label.clone());

    // /Library/LaunchDaemons 下的项属于系统级，普通用户改不了。
    let is_daemon = path
        .to_string_lossy()
        .contains("/Library/LaunchDaemons/");
    let (toggleable, reason) = if is_daemon {
        (
            false,
            Some("系统级守护进程，需要在「系统设置 → 通用 → 登录项与扩展」中管理".to_string()),
        )
    } else {
        (true, None)
    };

    Some(StartupItem {
        id: label,
        name,
        kind,
        enabled,
        developer: None,
        url: program,
        parent: obj
            .get("AssociatedBundleIdentifiers")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .map(str::to_string),
        toggleable,
        reason,
        bundle_path,
        icon_png_base64: None,
    })
}

/// 从 ProgramArguments 的某一项里解析 `.app` 路径。
fn bundle_from_program(value: &str) -> Option<String> {
    if !value.contains(".app") {
        return None;
    }
    let idx = value.find(".app")?;
    let bundle = &value[..idx + 4];
    if bundle.starts_with('/') && Path::new(bundle).is_dir() {
        Some(bundle.to_string())
    } else {
        None
    }
}

/// 当前用户的 uid，用于 `launchctl` 的 `gui/<uid>` 目标。
fn current_uid() -> u32 {
    // 不引入 libc：`id -u` 是所有 macOS 都有的系统命令。
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .unwrap_or(501)
}

/// 读取 `launchctl print-disabled` 的输出，返回 label → 是否禁用。
fn read_disabled_labels() -> std::collections::HashMap<String, bool> {
    use std::collections::HashMap;

    let mut map = HashMap::new();
    let uid = current_uid();
    let Ok(out) = Command::new("launchctl")
        .args(["print-disabled", &format!("gui/{uid}")])
        .output()
    else {
        return map;
    };
    if !out.status.success() {
        return map;
    }

    // 每行形如 `"label" => disabled` 或 `"label" => enabled`。
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let line = line.trim();
        let Some((label_part, state)) = line.split_once("=>") else {
            continue;
        };
        let label = label_part.trim().trim_matches('"');
        if label.is_empty() {
            continue;
        }
        let state = state.trim();
        if state == "disabled" {
            map.insert(label.to_string(), true);
        } else if state == "enabled" {
            map.insert(label.to_string(), false);
        }
    }
    map
}

/// 切换一条启动项的启用状态。
///
/// 走 `launchctl enable/disable`：这是用户域内 LaunchAgent 的官方开关，
/// 不需要 root，也不会绕过 Background Task Management 的记录。
/// 系统级守护进程（`/Library/LaunchDaemons`）不在本应用可改范围内，
/// 调用方应先检查 `toggleable`。
pub fn set_startup_item_enabled(id: &str, enabled: bool) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("启动项标识不能为空".to_string());
    }
    if id.contains('\n') || id.contains('\0') {
        return Err("启动项标识包含非法字符".to_string());
    }

    let uid = current_uid();
    let target = format!("gui/{uid}/{id}");
    let verb = if enabled { "enable" } else { "disable" };

    let out = Command::new("launchctl")
        .arg(verb)
        .arg(&target)
        .output()
        .map_err(|e| format!("调用 launchctl 失败：{e}"))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return Err(if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("launchctl {verb} 失败（退出码 {:?}）", out.status.code())
        });
    }

    Ok(())
}

/// 打开系统设置的「登录项与扩展」面板。
///
/// 系统托管的项（App 注册的后台任务、受保护的登录项）只能由用户在这里改，
/// 界面据此给出可点的入口而不是一个点了没反应的开关。
pub fn open_login_items_settings() -> Result<(), String> {
    let out = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.LoginItems-Settings.extension")
        .output()
        .map_err(|e| format!("打开系统设置失败：{e}"))?;

    if !out.status.success() {
        return Err("打开系统设置失败".to_string());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_launch_dirs_returns_well_formed_items() {
        // 真实机器上 launchd 目录里通常有若干 plist。
        // 不假设具体有哪些项，只验证扫描能跑通且字段自洽。
        let items = scan_launch_dirs();
        for item in &items {
            assert!(!item.id.is_empty(), "label 不应为空");
            assert!(!item.name.is_empty(), "展示名不应为空");
            if let Some(b) = &item.bundle_path {
                assert!(b.starts_with('/'), "bundle 路径应为绝对路径：{b}");
                assert!(b.ends_with(".app"), "bundle 路径应以 .app 结尾：{b}");
            }
            // 数据源是 plist 目录，分类只能是这两种。
            assert_ne!(item.kind, ItemKind::LoginItem, "目录扫描不该产出登录项");
        }
    }

    #[test]
    fn toggleable_items_have_no_reason() {
        // 可切换的项不应带拒绝原因，带了说明判定逻辑自相矛盾。
        let items = scan_launch_dirs();
        assert!(
            items
                .iter()
                .filter(|i| i.toggleable)
                .all(|i| i.reason.is_none()),
            "可切换的项不应带拒绝原因"
        );
    }

    #[test]
    fn daemons_are_not_toggleable() {
        // /Library/LaunchDaemons 下的项需要 root，必须标为不可切换，
        // 否则界面会给出一个点了必定失败的开关。
        let items = scan_launch_dirs();
        for item in &items {
            let is_daemon = item
                .url
                .as_deref()
                .map(|u| u.contains("/Library/LaunchDaemons"))
                .unwrap_or(false);
            if is_daemon {
                assert!(!item.toggleable, "{} 是守护进程，不该可切换", item.id);
                assert!(item.reason.is_some(), "不可切换就必须说明原因");
            }
        }
    }

    #[test]
    fn login_items_scan_does_not_panic() {
        // LSSharedFileList 在 BTM 存储为空时返回空列表，在 API 不可用时
        // 也要安全返回空。两条路径都不能 panic。
        let items = scan_login_items();
        for item in &items {
            assert_eq!(item.kind, ItemKind::LoginItem);
            assert!(item.id.starts_with("login-item:"), "登录项 id 应带前缀");
            assert!(item.enabled, "登录项只要在列表里就是启用状态");
            assert!(item.url.is_some(), "登录项应有可解析路径");
        }
    }

    #[test]
    fn list_startup_items_has_no_duplicate_ids() {
        // 同一项可能同时出现在多个目录，去重后不应有重复 id。
        let items = list_startup_items();
        let mut ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        let before = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(before, ids.len(), "列表不应包含重复 id");
    }

    #[test]
    fn list_startup_items_is_sorted_by_name() {
        let items = list_startup_items();
        for pair in items.windows(2) {
            assert!(
                pair[0].name.to_lowercase() <= pair[1].name.to_lowercase(),
                "列表应按名称排序：{} 出现在 {} 之前",
                pair[0].name,
                pair[1].name
            );
        }
    }

    #[test]
    fn rejects_empty_and_control_char_ids() {
        // 开关命令的入参校验：空、换行、NUL 都必须拒绝。
        assert!(set_startup_item_enabled("", true).is_err());
        assert!(set_startup_item_enabled("   ", true).is_err());
        assert!(set_startup_item_enabled("bad\nlabel", true).is_err());
        assert!(set_startup_item_enabled("bad\0label", true).is_err());
    }

    #[test]
    fn bundle_from_program_requires_existing_dir() {
        // 不存在的路径不该被当成 bundle（避免拿假路径去查图标）。
        assert!(bundle_from_program("/definitely/not/here.app").is_none());
        assert!(bundle_from_program("/usr/bin/open").is_none());
        assert!(bundle_from_program("relative/Foo.app").is_none());
    }
}
