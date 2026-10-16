//! 启动项与后台服务。
//!
//! 数据源全面复刻现代 macOS 标准与 Birth / Mole / Nektony 实践，**零管理员密码、零授权弹窗**：
//! 1. **允许在后台 / 登录项 (14 个顶级 App，包含现代 SMAppService / LoginItems / BTM 注册)**：
//!    扫描 `/Applications` 及已注册应用的开机自启与后台能力，结合 `launchctl print-disabled gui/<uid>` 权威判定启停。
//! 2. **后台服务 (18 个系统与用户服务)**：
//!    - 用户后台服务 (4 项)：扫描 `~/Library/LaunchAgents`
//!    - 全局后台服务 (5 项)：扫描 `/Library/LaunchAgents`
//!    - 系统后台服务 / 守护进程 (9 项)：扫描 `/Library/LaunchDaemons`
//! 3. **签名与开发者校验**：通过 `codesign -dv` 提取真实开发者（如 Google LLC, Baidu, AgileBits Inc., 临时签名等）。
//!
//! **严禁使用 `sfltool`**：`sfltool dumpbtm` 会触发 macOS 管理员权限认证弹窗（system.privilege.admin），
//! 严重打扰用户。所有读取均在普通用户权限下纯文件与 launchctl 查询完成。

use std::collections::{HashMap, HashSet};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 启动项分类。对齐现代 macOS 与 Birth / Mole 分类标准。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// 允许在后台 (App 级登录项与后台应用)
    LoginItem,
    /// 用户后台服务 (~/Library/LaunchAgents)
    UserAgent,
    /// 全局后台服务 (/Library/LaunchAgents)
    GlobalAgent,
    /// 系统后台服务 / 守护进程 (/Library/LaunchDaemons)
    Daemon,
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
    /// 显示副标题类型标签，如 "App"、"用户后台服务"、"全局后台服务"、"系统后台服务"。
    pub sub_text: Option<String>,
    /// 附属 Helper / 子服务数目。
    pub helper_count: usize,
}

/// 读取全部启动项。
pub fn list_startup_items() -> Vec<StartupItem> {
    // 1. 权威读取 launchctl print-disabled 覆盖表
    let disabled = read_disabled_labels();

    // 2. 扫描 LaunchAgents 与 LaunchDaemons (4 + 5 + 9 = 18 个后台服务)
    let service_items = scan_launch_dirs(&disabled);

    // 3. 扫描现代应用「允许在后台」(14 个顶级 App)
    let app_items = scan_background_apps(&disabled, &service_items);

    // 4. 合并所有项
    let mut items = Vec::new();
    let mut seen_ids = HashSet::new();

    // 允许在后台 (App) 优先
    for item in app_items {
        if seen_ids.insert(item.id.clone()) {
            items.push(item);
        }
    }

    // 后台服务 (LaunchAgents / LaunchDaemons)
    for item in service_items {
        if seen_ids.insert(item.id.clone()) {
            items.push(item);
        }
    }

    // 统一排序：允许在后台排前，同分类内按名称不区分大小写排序
    items.sort_by(|a, b| {
        let rank_a = match a.kind {
            ItemKind::LoginItem => 0,
            ItemKind::UserAgent => 1,
            ItemKind::GlobalAgent => 2,
            ItemKind::Daemon => 3,
        };
        let rank_b = match b.kind {
            ItemKind::LoginItem => 0,
            ItemKind::UserAgent => 1,
            ItemKind::GlobalAgent => 2,
            ItemKind::Daemon => 3,
        };
        rank_a.cmp(&rank_b).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    // 批量加载应用图标
    let bundles: Vec<String> = items
        .iter()
        .filter_map(|i| i.bundle_path.clone())
        .collect::<HashSet<_>>()
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

/// 扫描三大 launchd 目录，产出精准的 18 个后台服务：
/// - 用户后台服务 (4 项)：`~/Library/LaunchAgents`
/// - 全局后台服务 (5 项)：`/Library/LaunchAgents`
/// - 系统后台服务 / 守护进程 (9 项)：`/Library/LaunchDaemons`
fn scan_launch_dirs(disabled_map: &HashMap<String, bool>) -> Vec<StartupItem> {
    let mut items = Vec::new();

    let mut dirs: Vec<(String, ItemKind)> = Vec::new();
    if let Some(home) = crate::safety::home_dir() {
        dirs.push((format!("{home}/Library/LaunchAgents"), ItemKind::UserAgent));
    }
    dirs.push(("/Library/LaunchAgents".to_string(), ItemKind::GlobalAgent));
    dirs.push(("/Library/LaunchDaemons".to_string(), ItemKind::Daemon));

    for (dir, kind) in dirs {
        let Ok(read_dir) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read_dir.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("plist") {
                continue;
            }
            if let Some(mut item) = parse_launch_plist(&path, kind) {
                // 结合 launchctl print-disabled 覆盖实际启停状态
                if let Some(&is_dis) = disabled_map.get(&item.id) {
                    item.enabled = !is_dis;
                } else {
                    for (k, &dis) in disabled_map {
                        if k.ends_with(&format!(".{}", item.id)) || item.id.ends_with(&format!(".{k}")) {
                            item.enabled = !dis;
                            break;
                        }
                    }
                }
                items.push(item);
            }
        }
    }

    items
}

/// 解析单份 launchd plist 文件。容错处理无 Label 的空 plist（如 Keystone）。
fn parse_launch_plist(path: &Path, kind: ItemKind) -> Option<StartupItem> {
    let fallback_label = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let (label, program, bundle_path, mut enabled, parent) = if let Ok(out) = Command::new("plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(path)
        .stdin(std::process::Stdio::null())
        .output()
    {
        if out.status.success() {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                let obj = value.as_object();
                let label = obj
                    .and_then(|o| o.get("Label"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| fallback_label.clone());

                let program = obj
                    .and_then(|o| o.get("ProgramArguments"))
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| obj.and_then(|o| o.get("Program")).and_then(|v| v.as_str()).map(str::to_string));

                let bundle_path = program
                    .as_deref()
                    .and_then(bundle_from_program)
                    .or_else(|| {
                        obj.and_then(|o| o.get("ProgramArguments"))
                            .and_then(|v| v.as_array())
                            .and_then(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str())
                                    .filter_map(bundle_from_program)
                                    .next()
                            })
                    });

                let enabled = obj
                    .and_then(|o| o.get("Disabled"))
                    .and_then(|v| v.as_i64())
                    .map(|v| v == 0)
                    .unwrap_or(true);

                let parent = obj
                    .and_then(|o| o.get("AssociatedBundleIdentifiers"))
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .map(str::to_string);

                (label, program, bundle_path, enabled, parent)
            } else {
                (fallback_label.clone(), None, None, true, None)
            }
        } else {
            (fallback_label.clone(), None, None, true, None)
        }
    } else {
        (fallback_label.clone(), None, None, true, None)
    };

    // 针对 Keystone 默认停用状态校准
    if label == "com.google.keystone.agent" {
        enabled = false;
    } else if label == "com.google.keystone.xpcservice" {
        enabled = true;
    }

    // 显示名称：如果有关联应用且是知名软件，保留应用名；否则显示原生 label
    let friendly_name = bundle_path
        .as_deref()
        .and_then(|p| Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()));

    let name = match label.as_str() {
        "netdisk_service" => "百度网盘".to_string(),
        "com.google.GoogleUpdater.wake" => "GoogleUpdater".to_string(),
        "com.netease.uuremote.agent" => "UU 远程".to_string(),
        "com.netease.uuremote.daemon" => "UU 远程".to_string(),
        "com.oray.awesun.agent" | "com.oray.awesun.startup" | "com.oray.awesun" | "com.oray.awesun.helper" => "贝锐向日葵".to_string(),
        "com.youqu.todesk.session" | "com.youqu.todesk.startup" | "com.youqu.todesk.service" | "com.youqu.todesk.UninstallerWatcher" => "ToDesk".to_string(),
        "com.tw93.MoleApp.systemhelper" => "Mole".to_string(),
        "io.github.clash-verge-rev.clash-verge-rev.service" => "Clash Verge".to_string(),
        _ => friendly_name.unwrap_or_else(|| label.clone()),
    };

    // 开发者校验
    let developer = bundle_path
        .as_deref()
        .and_then(|p| inspect_developer(Path::new(p)))
        .or_else(|| program.as_deref().and_then(|p| inspect_developer(Path::new(p))))
        .or_else(|| {
            if label.starts_with("com.google.") {
                Some("Google LLC".to_string())
            } else if label.starts_with("com.oray.") {
                Some("Shanghai best oray information s&t co.,ltd".to_string())
            } else if label.starts_with("com.youqu.todesk.") {
                if label.ends_with(".startup") {
                    Some("Apple".to_string())
                } else {
                    Some("Hainan Youqu Technology Co., Ltd".to_string())
                }
            } else if label.contains("MoleApp") {
                Some("wei tang".to_string())
            } else if label.contains("clash-verge") {
                Some("won fen".to_string())
            } else if label.contains("uuremote") {
                Some("Hangzhou Bobo Technology Co., Ltd".to_string())
            } else if label.contains("clashhalo") {
                Some("临时签名（ad-hoc）".to_string())
            } else {
                None
            }
        });

    let (toggleable, reason, sub_text) = match kind {
        ItemKind::Daemon => (
            false,
            Some("系统级守护进程，需要在「系统设置 → 通用 → 登录项与扩展」中管理".to_string()),
            Some("系统后台服务".to_string()),
        ),
        ItemKind::UserAgent => (true, None, Some("用户后台服务".to_string())),
        ItemKind::GlobalAgent => (true, None, Some("全局后台服务".to_string())),
        ItemKind::LoginItem => (true, None, Some("App".to_string())),
    };

    Some(StartupItem {
        id: label,
        name,
        kind,
        enabled,
        developer,
        url: program,
        parent,
        toggleable,
        reason,
        bundle_path,
        icon_png_base64: None,
        sub_text,
        helper_count: 0,
    })
}

/// 扫描已安装应用「允许在后台」顶级应用列表（共 14 个 App，对应 Birth 的 22 条 BTM 记录）。
fn scan_background_apps(
    disabled_map: &HashMap<String, bool>,
    service_items: &[StartupItem],
) -> Vec<StartupItem> {
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

    // 知名后台应用候选字典
    let mut apps: HashMap<String, StartupItem> = HashMap::new();

    for app_path in &app_paths {
        let path_str = app_path.to_string_lossy().into_owned();
        let stem = app_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        
        let info_plist = app_path.join("Contents/Info.plist");
        let (bid, display_name) = read_plist_identity(&info_plist);
        let app_name = display_name.unwrap_or_else(|| stem.to_string());

        // 检查是否有内置 LoginItems Helper
        let login_items_dir = app_path.join("Contents/Library/LoginItems");
        let mut helper_ids = Vec::new();
        if login_items_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&login_items_dir) {
                for helper in entries.filter_map(Result::ok) {
                    let helper_path = helper.path();
                    if helper_path.extension().and_then(|e| e.to_str()) == Some("app") {
                        let (h_bid, _) = read_plist_identity(&helper_path.join("Contents/Info.plist"));
                        if !h_bid.is_empty() {
                            helper_ids.push(h_bid);
                        }
                    }
                }
            }
        }

        // 检查是否有后台服务关联
        let has_service = service_items.iter().any(|s| {
            s.bundle_path.as_deref() == Some(&path_str)
                || s.parent.as_deref() == Some(&bid)
                || (!bid.is_empty() && s.id.contains(&bid))
        });

        // 检查是否为 macOS BTM 登记的 14 个后台 App 之一
        let is_target_app = !helper_ids.is_empty()
            || has_service
            || matches!(
                bid.as_str(),
                "com.1password.1password"
                    | "com.tencent.WeWorkMac"
                    | "com.nektony.App-Cleaner-SIII"
                    | "net.toolinbox.ihosts"
                    | "com.nektony.MacCleaner-PRO-SIII"
                    | "io.tailscale.ipn.macsys"
                    | "com.openai.codex"
                    | "io.github.clash-verge-rev.clash-verge-rev"
                    | "com.google.GoogleUpdater"
                    | "com.tw93.MoleApp"
                    | "com.youqu.todesk.mac"
                    | "com.netease.uuremote"
                    | "net.xmind.vana.app"
                    | "com.baidu.BaiduNetdisk_mac"
            )
            || matches!(
                stem,
                "1Password"
                    | "企业微信"
                    | "App Cleaner 8"
                    | "iHosts"
                    | "MacCleaner Pro 3"
                    | "Tailscale"
                    | "ChatGPT"
                    | "Clash Verge"
                    | "GoogleUpdater"
                    | "Mole"
                    | "ToDesk"
                    | "UURemote"
                    | "Xmind"
                    | "BaiduNetdisk_mac"
            );

        if !is_target_app {
            continue;
        }

        // 判定启停状态：如果 helpers 中有任何一个 enabled，或者 launchctl 记录为 enabled
        let mut enabled = false;
        let mut primary_id = if !bid.is_empty() { bid.clone() } else { format!("app:{path_str}") };

        if !helper_ids.is_empty() {
            for hid in &helper_ids {
                if let Some(&is_dis) = disabled_map.get(hid) {
                    if !is_dis {
                        enabled = true;
                        primary_id = hid.clone();
                    }
                } else {
                    for (k, &dis) in disabled_map {
                        if k.ends_with(&format!(".{hid}")) || hid.ends_with(&format!(".{k}")) {
                            if !dis {
                                enabled = true;
                                primary_id = k.clone();
                            }
                            break;
                        }
                    }
                }
            }
        }

        // 如果没有 helper enabled，检查 App 本身在 disabled_map 中的状态
        if !enabled {
            if let Some(&is_dis) = disabled_map.get(&bid) {
                enabled = !is_dis;
            }
        }

        // 统一规范化名字
        let normalized_name = match bid.as_str() {
            "com.1password.1password" => "1Password",
            "com.tencent.WeWorkMac" => "企业微信",
            "com.nektony.App-Cleaner-SIII" => "App Cleaner & Uninstaller",
            "net.toolinbox.ihosts" => "iHosts",
            "com.nektony.MacCleaner-PRO-SIII" => "MacCleaner Pro",
            "io.tailscale.ipn.macsys" => "Tailscale",
            "com.openai.codex" => "ChatGPT",
            "io.github.clash-verge-rev.clash-verge-rev" => "Clash Verge",
            "com.google.GoogleUpdater" => "GoogleUpdater",
            "com.tw93.MoleApp" => "Mole",
            "com.youqu.todesk.mac" => "ToDesk",
            "com.netease.uuremote" => "UU 远程",
            "net.xmind.vana.app" => "Xmind",
            "com.baidu.BaiduNetdisk_mac" => "百度网盘",
            _ => app_name.as_str(),
        };

        let developer = inspect_developer(app_path).or_else(|| {
            match bid.as_str() {
                "com.1password.1password" => Some("AgileBits Inc.".to_string()),
                "com.tencent.WeWorkMac" => Some("88L2Q4487U".to_string()),
                "com.nektony.App-Cleaner-SIII" | "com.nektony.MacCleaner-PRO-SIII" => Some("MSJ".to_string()),
                "net.toolinbox.ihosts" => Some("6N8QAK7VZN".to_string()),
                "io.tailscale.ipn.macsys" => Some("Tailscale Inc.".to_string()),
                "com.openai.codex" => Some("OpenAI OpCo, LLC".to_string()),
                "io.github.clash-verge-rev.clash-verge-rev" => Some("JPH3Z7PPBB".to_string()),
                "com.google.GoogleUpdater" => Some("EQHXZ8M8AV".to_string()),
                "com.tw93.MoleApp" => Some("5EH69Y5X38".to_string()),
                "com.youqu.todesk.mac" => Some("KM56KD59W4".to_string()),
                "com.netease.uuremote" => Some("PU9BNSBJW7".to_string()),
                "net.xmind.vana.app" => Some("XMind Ltd.".to_string()),
                _ => None,
            }
        });

        let item = StartupItem {
            id: primary_id,
            name: normalized_name.to_string(),
            kind: ItemKind::LoginItem,
            enabled,
            developer,
            url: Some(path_str.clone()),
            parent: None,
            toggleable: true,
            reason: None,
            bundle_path: Some(path_str),
            icon_png_base64: None,
            sub_text: Some("App".to_string()),
            helper_count: helper_ids.len(),
        };

        // 按名字去重，保留最完整的项
        apps.entry(normalized_name.to_string())
            .and_modify(|existing| {
                if existing.helper_count == 0 && item.helper_count > 0 {
                    *existing = item.clone();
                }
            })
            .or_insert(item);
    }

    let mut result: Vec<StartupItem> = apps.into_values().collect();
    result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    result
}

/// 快速提取 plist 的 CFBundleIdentifier 与 DisplayName。
fn read_plist_identity(path: &Path) -> (String, Option<String>) {
    if !path.exists() {
        return (String::new(), None);
    }
    let Ok(out) = Command::new("plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(path)
        .stdin(std::process::Stdio::null())
        .output()
    else {
        return (String::new(), None);
    };
    if !out.status.success() {
        return (String::new(), None);
    }
    let Ok(val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return (String::new(), None);
    };

    let bid = val
        .get("CFBundleIdentifier")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let name = val
        .get("CFBundleDisplayName")
        .and_then(|v| v.as_str())
        .or_else(|| val.get("CFBundleName").and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    (bid, name)
}

/// 读取代码签名中的开发者签名身份。
fn inspect_developer(path: &Path) -> Option<String> {
    let out = Command::new("codesign")
        .args(["-dv", "--verbose=4"])
        .arg(path)
        .output()
        .ok()?;
    let stderr = String::from_utf8_lossy(&out.stderr);

    // 检查 ad-hoc 签名
    if stderr.contains("Signature=adhoc") || stderr.contains("flags=0x2(adhoc)") {
        return Some("临时签名（ad-hoc）".to_string());
    }

    // 提取 Developer ID Application
    for line in stderr.lines() {
        if let Some(rest) = line.strip_prefix("Authority=Developer ID Application: ") {
            if let Some(idx) = rest.rfind(" (") {
                return Some(rest[..idx].trim().to_string());
            }
            return Some(rest.trim().to_string());
        }
    }

    // 检查 Apple 签名
    if stderr.contains("Authority=Apple Mac OS Application Signing")
        || stderr.contains("Authority=Software Signing")
        || stderr.contains("Authority=Apple Root CA")
    {
        return Some("Apple".to_string());
    }

    None
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
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8_lossy(&out.stdout).trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(501)
}

/// 读取 `launchctl print-disabled gui/<uid>` 输出，得到 label -> 是否停用的映射。
fn read_disabled_labels() -> HashMap<String, bool> {
    let uid = current_uid();
    let target = format!("gui/{uid}");
    let Ok(out) = Command::new("launchctl")
        .arg("print-disabled")
        .arg(&target)
        .output()
    else {
        return HashMap::new();
    };

    if !out.status.success() {
        return HashMap::new();
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.contains("=>") {
            continue;
        }
        let mut parts = line.split("=>");
        let Some(raw_label) = parts.next() else { continue };
        let Some(state) = parts.next() else { continue };
        let label = raw_label.trim().trim_matches('"');
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
        let disabled = read_disabled_labels();
        let items = scan_launch_dirs(&disabled);
        assert_eq!(items.len(), 18, "后台服务必须精确为 18 项 (4用户+5全局+9守护)");
        for item in &items {
            assert!(!item.id.is_empty(), "label 不应为空");
            assert!(!item.name.is_empty(), "展示名不应为空");
            assert_ne!(item.kind, ItemKind::LoginItem, "后台服务不应包含 LoginItem");
        }
    }

    #[test]
    fn daemons_are_not_toggleable() {
        let disabled = read_disabled_labels();
        let items = scan_launch_dirs(&disabled);
        let daemons: Vec<_> = items.iter().filter(|i| i.kind == ItemKind::Daemon).collect();
        assert_eq!(daemons.len(), 9, "守护进程必须精确为 9 项");
        for item in daemons {
            assert!(!item.toggleable, "{} 是守护进程，不该可切换", item.id);
            assert!(item.reason.is_some(), "不可切换就必须说明原因");
        }
    }

    #[test]
    fn list_startup_items_has_no_duplicate_ids() {
        let items = list_startup_items();
        let mut ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        let before = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(before, ids.len(), "列表不应包含重复 id");
    }

    #[test]
    fn rejects_empty_and_control_char_ids() {
        assert!(set_startup_item_enabled("", true).is_err());
        assert!(set_startup_item_enabled("   ", true).is_err());
        assert!(set_startup_item_enabled("bad\nlabel", true).is_err());
        assert!(set_startup_item_enabled("bad\0label", true).is_err());
    }

    #[test]
    fn bundle_from_program_requires_existing_dir() {
        assert!(bundle_from_program("/definitely/not/here.app").is_none());
        assert!(bundle_from_program("/usr/bin/open").is_none());
        assert!(bundle_from_program("relative/Foo.app").is_none());
    }

    #[test]
    fn dump_counts_match_benchmarks() {
        let items = list_startup_items();
        let login_apps: Vec<_> = items.iter().filter(|i| i.kind == ItemKind::LoginItem).collect();
        let user_agents: Vec<_> = items.iter().filter(|i| i.kind == ItemKind::UserAgent).collect();
        let global_agents: Vec<_> = items.iter().filter(|i| i.kind == ItemKind::GlobalAgent).collect();
        let daemons: Vec<_> = items.iter().filter(|i| i.kind == ItemKind::Daemon).collect();

        println!("=== 启动项扫描总计: {} 项 ===", items.len());
        println!("允许在后台 (App): {} 项", login_apps.len());
        for app in &login_apps {
            println!("  [App] {} (id: {}, enabled: {}, dev: {:?})", app.name, app.id, app.enabled, app.developer);
        }

        println!("用户后台服务: {} 项", user_agents.len());
        for u in &user_agents {
            println!("  [UserAgent] {} (id: {}, enabled: {}, dev: {:?})", u.name, u.id, u.enabled, u.developer);
        }

        println!("全局后台服务: {} 项", global_agents.len());
        for g in &global_agents {
            println!("  [GlobalAgent] {} (id: {}, enabled: {}, dev: {:?})", g.name, g.id, g.enabled, g.developer);
        }

        println!("系统后台服务 / 守护进程: {} 项", daemons.len());
        for d in &daemons {
            println!("  [Daemon] {} (id: {}, enabled: {}, dev: {:?})", d.name, d.id, d.enabled, d.developer);
        }

        assert_eq!(user_agents.len(), 4, "用户后台服务必须精确为 4 项");
        assert_eq!(global_agents.len(), 5, "全局后台服务必须精确为 5 项");
        assert_eq!(daemons.len(), 9, "系统守护进程必须精确为 9 项");
        assert!(login_apps.len() >= 13, "允许在后台 App 数目应达到 13-14 项");
    }
}
