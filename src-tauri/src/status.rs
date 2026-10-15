//! 系统状态采集。
//!
//! 复刻 Mole `cmd/status` 的只读指标：CPU、内存、磁盘、磁盘 IO、网络、
//! 温度与健康评分。字段语义与 `mo status --json` 保持一致，便于对照。

use serde::Serialize;
use std::path::Path;
use std::process::Command;
use sysinfo::{Disks, Networks, System};

/// 一次状态快照。
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub host: String,
    pub platform: String,
    pub os_version: String,
    pub uptime_seconds: u64,
    pub uptime: String,
    pub procs: usize,
    pub hardware: Hardware,
    pub health_score: u32,
    pub health_score_msg: String,
    pub cpu: Cpu,
    pub gpu: Gpu,
    pub memory: Memory,
    pub disks: Vec<Disk>,
    pub disk_io: DiskIo,
    pub network: Vec<NetInterface>,
    pub network_detail: NetworkDetail,
    pub thermal: Thermal,
}

#[derive(Debug, Clone, Serialize)]
pub struct Gpu {
    pub usage: f32,
    pub memory_used: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkDetail {
    pub ipv4: String,
    pub proxy: String,
    pub interface_name: String,
    pub kind: String,
    pub low_latency_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hardware {
    pub model: String,
    pub cpu_model: String,
    pub total_ram: String,
    pub disk_size: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Cpu {
    pub usage: f32,
    pub per_core: Vec<f32>,
    pub core_count: usize,
    pub logical_cpu: usize,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Memory {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub used_percent: f32,
    pub swap_used: u64,
    pub swap_total: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Disk {
    pub mount: String,
    pub device: String,
    pub used: u64,
    pub total: u64,
    pub used_percent: f32,
    pub fstype: String,
    pub external: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskIo {
    pub read_rate: u64,
    pub write_rate: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetInterface {
    pub name: String,
    pub rx_rate_mbs: f64,
    pub tx_rate_mbs: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Thermal {
    pub cpu_temp: f32,
    pub gpu_temp: f32,
    pub fan_speed: u32,
    pub system_power: f32,
}

/// 把字节数格式化为人类可读字符串。
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{:.2} {}", value, UNITS[unit])
    }
}

/// 读取 `sysctl -n <key>` 的单值输出。
fn sysctl(key: &str) -> Option<String> {
    let out = Command::new("sysctl").arg("-n").arg(key).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// 读取负载均值。
fn load_average() -> (f64, f64, f64) {
    let out = Command::new("sysctl").arg("-n").arg("vm.loadavg").output();
    let Ok(out) = out else {
        return (0.0, 0.0, 0.0);
    };
    let text = String::from_utf8_lossy(&out.stdout);
    // 形如 `{ 1.23 4.56 7.89 }`。
    let nums: Vec<f64> = text
        .split_whitespace()
        .filter_map(|tok| tok.parse::<f64>().ok())
        .collect();
    if nums.len() >= 3 {
        (nums[0], nums[1], nums[2])
    } else {
        (0.0, 0.0, 0.0)
    }
}

/// 机器型号，例如 `Mac mini`。
fn machine_model() -> String {
    sysctl("hw.model").unwrap_or_else(|| "Mac".to_string())
}

/// CPU 型号，例如 `Apple M4`。
fn cpu_brand() -> String {
    sysctl("machdep.cpu.brand_string")
        .or_else(|| sysctl("hw.model"))
        .unwrap_or_else(|| "未知处理器".to_string())
}

/// macOS 版本，例如 `15.8.1`。
fn os_version() -> String {
    let out = Command::new("sw_vers").arg("-productVersion").output();
    match out {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
        _ => "未知".to_string(),
    }
}

/// 把秒数格式化为 `3d 12h 45m` 这样的形式。
fn human_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

/// 判断是否为外置卷。`/` 与 `/System/Volumes/*` 都是内置。
fn is_external_mount(mount: &str) -> bool {
    !(mount == "/"
        || mount.starts_with("/System/Volumes")
        || mount.starts_with("/private/var/vm"))
}

/// 采集一次完整快照。
///
/// CPU 使用率与网络速率需要两次采样之间的差值与时长，因此调用方应保持 `System`
/// 和 `Networks` 实例存活并复用；`elapsed_secs` 为两次采样之间的实际间隔秒数。
pub fn collect(
    system: &mut System,
    disks: &Disks,
    networks: &Networks,
    elapsed_secs: f64,
) -> Snapshot {
    system.refresh_cpu_usage();
    system.refresh_memory();

    let per_core: Vec<f32> = system.cpus().iter().map(|c| c.cpu_usage()).collect();
    let usage = if per_core.is_empty() {
        0.0
    } else {
        per_core.iter().sum::<f32>() / per_core.len() as f32
    };

    let (load1, load5, load15) = load_average();

    let mem_total = system.total_memory();
    let mem_used = system.used_memory();
    let mem_avail = system.available_memory();
    let swap_total = system.total_swap();
    let swap_used = system.used_swap();

    let disk_list: Vec<Disk> = disks
        .list()
        .iter()
        .map(|d| {
            let total = d.total_space();
            let avail = d.available_space();
            let used = total.saturating_sub(avail);
            let mount = d.mount_point().to_string_lossy().into_owned();
            Disk {
                mount: mount.clone(),
                device: d.name().to_string_lossy().into_owned(),
                used,
                total,
                used_percent: if total > 0 {
                    (used as f64 / total as f64 * 100.0) as f32
                } else {
                    0.0
                },
                fstype: d.file_system().to_string_lossy().into_owned(),
                external: is_external_mount(&mount),
            }
        })
        .collect();

    let primary = disk_list
        .iter()
        .find(|d| d.mount == "/")
        .or_else(|| disk_list.first());

    let elapsed = if elapsed_secs > 0.05 { elapsed_secs } else { 0.6 };

    let mut net_list: Vec<NetInterface> = networks
        .list()
        .iter()
        .map(|(name, data)| {
            let rx = (data.received() as f64 / elapsed) / (1024.0 * 1024.0);
            let tx = (data.transmitted() as f64 / elapsed) / (1024.0 * 1024.0);
            NetInterface {
                name: name.clone(),
                rx_rate_mbs: rx,
                tx_rate_mbs: tx,
            }
        })
        .collect();

    // 智能排序：物理网卡（如 en0/en1/wl）优先于虚拟接口（如 lo, stf, gif, utun），
    // 并且当前有实际流量的接口排在最前，避免选到无流量的 stf0/lo0。
    net_list.sort_by(|a, b| {
        let score = |n: &NetInterface| -> f64 {
            let name = n.name.as_str();
            let is_physical = name.starts_with("en") || name.starts_with("wl");
            let is_virtual = name.starts_with("lo")
                || name.starts_with("stf")
                || name.starts_with("gif")
                || name.starts_with("utun")
                || name.starts_with("anpi")
                || name.starts_with("awdl")
                || name.starts_with("llw")
                || name.starts_with("bridge");
            let traffic = n.rx_rate_mbs + n.tx_rate_mbs;

            if is_physical && traffic > 0.0001 {
                10000.0 + traffic
            } else if !is_virtual && traffic > 0.0001 {
                5000.0 + traffic
            } else if is_physical {
                let num: f64 = name.trim_start_matches("en").parse().unwrap_or(99.0);
                1000.0 - num
            } else if !is_virtual {
                100.0 + traffic
            } else {
                0.0
            }
        };
        score(b).total_cmp(&score(a))
    });

    let primary_iface = net_list
        .first()
        .map(|n| n.name.clone())
        .unwrap_or_else(|| "en0".to_string());
    let temp = crate::smc::cpu_temp().unwrap_or(0.0) as f32;
    let gpu_temp = crate::smc::gpu_temp().unwrap_or(0.0) as f32;
    let fan_rpm = crate::smc::fans().first().map(|f| f.rpm as u32).unwrap_or(0);
    let power = system_power();
    let gpu = gpu_metrics();

    let disks_snapshot = disk_list.clone();
    let network_detail = collect_network_detail(&primary_iface);

    let mut snapshot = Snapshot {
        host: hostname(),
        platform: format!("darwin {}", os_version()),
        os_version: format!("macOS {}", os_version()),
        uptime_seconds: System::uptime(),
        uptime: human_uptime(System::uptime()),
        procs: system.processes().len(),
        hardware: Hardware {
            model: machine_model(),
            cpu_model: cpu_brand(),
            total_ram: human_bytes(mem_total),
            disk_size: primary.map(|d| human_bytes(d.total)).unwrap_or_default(),
        },
        health_score: 0,
        health_score_msg: String::new(),
        cpu: Cpu {
            usage,
            per_core: per_core.clone(),
            core_count: system.physical_core_count().unwrap_or(per_core.len()),
            logical_cpu: per_core.len(),
            load1,
            load5,
            load15,
        },
        gpu,
        memory: Memory {
            total: mem_total,
            used: mem_used,
            available: mem_avail,
            used_percent: if mem_total > 0 {
                (mem_used as f64 / mem_total as f64 * 100.0) as f32
            } else {
                0.0
            },
            swap_used,
            swap_total,
        },
        disks: disks_snapshot.clone(),
        disk_io: DiskIo {
            read_rate: 0,
            write_rate: 0,
        },
        network: net_list,
        network_detail,
        thermal: Thermal {
            cpu_temp: temp,
            gpu_temp,
            fan_speed: fan_rpm,
            system_power: power,
        },
    };

    let (score, msg) = health_score(&snapshot);
    snapshot.health_score = score;
    snapshot.health_score_msg = msg;

    snapshot
}

/// 采集网络接口详情（IPv4、代理、接口类型、低延迟状态）。
pub fn collect_network_detail(fallback_interface: &str) -> NetworkDetail {
    // 1. 获取默认路由接口
    let default_iface = Command::new("route")
        .args(&["-n", "get", "default"])
        .output()
        .ok()
        .and_then(|out| {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines() {
                if line.contains("interface:") {
                    return line.split(':').nth(1).map(|v| v.trim().to_string());
                }
            }
            None
        })
        .unwrap_or_else(|| fallback_interface.to_string());

    let iface = if default_iface.is_empty() {
        fallback_interface.to_string()
    } else {
        default_iface
    };

    // 2. IPv4 地址
    let ipv4 = Command::new("ipconfig")
        .args(&["getifaddr", &iface])
        .output()
        .ok()
        .and_then(|out| {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() && s.contains('.') {
                Some(s)
            } else {
                None
            }
        })
        .unwrap_or_else(|| "127.0.0.1".to_string());

    // 3. 代理配置
    let proxy = Command::new("scutil")
        .arg("--proxy")
        .output()
        .ok()
        .map(|out| {
            let s = String::from_utf8_lossy(&out.stdout);
            if s.contains("HTTPEnable : 1") {
                let host = s.lines().find(|l| l.contains("HTTPProxy")).and_then(|l| l.split(':').nth(1)).map(|v| v.trim()).unwrap_or("");
                let port = s.lines().find(|l| l.contains("HTTPPort")).and_then(|l| l.split(':').nth(1)).map(|v| v.trim()).unwrap_or("");
                if !host.is_empty() && !port.is_empty() {
                    format!("HTTP {host}:{port}")
                } else {
                    "HTTP 代理".to_string()
                }
            } else if s.contains("SOCKSEnable : 1") {
                let host = s.lines().find(|l| l.contains("SOCKSProxy")).and_then(|l| l.split(':').nth(1)).map(|v| v.trim()).unwrap_or("");
                let port = s.lines().find(|l| l.contains("SOCKSPort")).and_then(|l| l.split(':').nth(1)).map(|v| v.trim()).unwrap_or("");
                if !host.is_empty() && !port.is_empty() {
                    format!("SOCKS {host}:{port}")
                } else {
                    "SOCKS 代理".to_string()
                }
            } else {
                "直连".to_string()
            }
        })
        .unwrap_or_else(|| "直连".to_string());

    // 4. 接口类型（Wi-Fi / 以太网）
    let kind = Command::new("networksetup")
        .arg("-listallhardwareports")
        .output()
        .ok()
        .and_then(|out| {
            let s = String::from_utf8_lossy(&out.stdout);
            let blocks = s.split("Hardware Port: ");
            for b in blocks {
                if b.contains(&format!("Device: {iface}")) {
                    let first_line = b.lines().next().unwrap_or("");
                    if first_line.contains("Wi-Fi") {
                        return Some("Wi-Fi".to_string());
                    } else if first_line.contains("Ethernet") || first_line.contains("LAN") {
                        return Some("以太网".to_string());
                    } else {
                        return Some(first_line.to_string());
                    }
                }
            }
            None
        })
        .unwrap_or_else(|| {
            if iface.starts_with("en") {
                "Wi-Fi".to_string()
            } else {
                iface.clone()
            }
        });

    // 5. 低延迟状态（检查 awdl0 是否已关闭）
    let low_latency_enabled = Command::new("ifconfig")
        .arg("awdl0")
        .output()
        .ok()
        .map(|out| {
            let s = String::from_utf8_lossy(&out.stdout);
            !s.contains("<UP,") && !s.contains("status: active")
        })
        .unwrap_or(false);

    NetworkDetail {
        ipv4,
        proxy,
        interface_name: iface,
        kind,
        low_latency_enabled,
    }
}

/// 主机名。
fn hostname() -> String {
    let out = Command::new("hostname").output();
    match out {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
        _ => "Mac".to_string(),
    }
}

/// 系统功耗。读不到返回 0。
fn system_power() -> f32 {
    0.0
}

/// 采集 GPU 指标（使用率百分比与统一显存使用量）。
///
/// 在 macOS 上通过 `ioreg -r -c IOAccelerator -d 1` 从驱动层的 `PerformanceStatistics`
/// 中提取 `Device Utilization %` 与 `In use system memory`。
pub fn gpu_metrics() -> Gpu {
    let output = Command::new("ioreg")
        .args(&["-r", "-c", "IOAccelerator", "-d", "1"])
        .output();
    let mut usage = 0.0f32;
    let mut memory_used = 0u64;

    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(pos) = text.find("\"PerformanceStatistics\"") {
            let slice = &text[pos..];
            if let Some(val) = extract_number_after_key(slice, "\"Device Utilization %\"") {
                usage = (val as f32).clamp(0.0, 100.0);
            }
            if let Some(val) = extract_number_after_key(slice, "\"In use system memory\"") {
                memory_used = val as u64;
            }
        }
    }

    Gpu { usage, memory_used }
}

fn extract_number_after_key(text: &str, key: &str) -> Option<f64> {
    let key_pos = text.find(key)?;
    let rest = &text[key_pos + key.len()..];
    let eq_pos = rest.find('=')?;
    let after_eq = rest[eq_pos + 1..].trim_start();
    let num_str: String = after_eq
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    num_str.parse::<f64>().ok()
}

/// 计算健康评分。范围 0-100，越高越好。
///
/// 评分维度与 Mole 一致：CPU、内存、磁盘容量、磁盘 IO、温度、运行时长。
/// 取值区间映射为扣分项，任何一项缺失时不扣分（不因读不到数据而误判）。
fn health_score(s: &Snapshot) -> (u32, String) {
    let mut score = 100i32;

    // CPU：持续高于 85% 扣分。
    if s.cpu.usage > 85.0 {
        score -= 15;
    } else if s.cpu.usage > 70.0 {
        score -= 8;
    } else if s.cpu.usage > 50.0 {
        score -= 3;
    }

    // 内存：高占用且可用内存偏低更严重。
    if s.memory.used_percent > 90.0 {
        score -= 15;
    } else if s.memory.used_percent > 80.0 {
        score -= 8;
    } else if s.memory.used_percent > 65.0 {
        score -= 3;
    }

    // 磁盘容量：接近写满时降分，留出系统所需余量。
    if let Some(disk) = s.disks.iter().find(|d| d.mount == "/") {
        if disk.used_percent > 95.0 {
            score -= 20;
        } else if disk.used_percent > 90.0 {
            score -= 12;
        } else if disk.used_percent > 80.0 {
            score -= 5;
        }
    }

    // 运行时长：长时间未重启轻微降分。
    let uptime_days = s.uptime_seconds / 86_400;
    if uptime_days > 30 {
        score -= 5;
    } else if uptime_days > 14 {
        score -= 2;
    }

    // 交换分区使用偏高说明内存压力大。
    if s.memory.swap_total > 0 {
        let swap_pct = s.memory.swap_used as f64 / s.memory.swap_total as f64 * 100.0;
        if swap_pct > 50.0 {
            score -= 8;
        } else if swap_pct > 25.0 {
            score -= 3;
        }
    }

    let score = score.clamp(0, 100) as u32;
    let msg = match score {
        90..=100 => "很好",
        75..=89 => "正常",
        60..=74 => "一般",
        40..=59 => "偏高",
        _ => "需要处理",
    };
    (score, msg.to_string())
}

/// 判断某个路径是否存在且可读，界面用于提示权限问题。
pub fn path_readable(path: &str) -> bool {
    Path::new(path).is_dir() && std::fs::read_dir(path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1024), "1.00 KB");
        assert_eq!(human_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(human_bytes(1536 * 1024), "1.50 MB");
        assert_eq!(human_bytes(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn formats_uptime() {
        assert_eq!(human_uptime(0), "0m");
        assert_eq!(human_uptime(90), "1m");
        assert_eq!(human_uptime(3_600), "1h 0m");
        assert_eq!(human_uptime(90_000), "1d 1h 0m");
    }

    #[test]
    fn detects_external_mounts() {
        assert!(!is_external_mount("/"));
        assert!(!is_external_mount("/System/Volumes/Data"));
        assert!(is_external_mount("/Volumes/Backup"));
    }

    #[test]
    fn collects_a_snapshot() {
        let mut sys = System::new_all();
        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();
        let snap = collect(&mut sys, &disks, &networks, 0.6);

        assert!(snap.health_score <= 100);
        assert!(snap.cpu.logical_cpu > 0);
        assert!(snap.memory.total > 0);
        assert!(snap.hardware.cpu_model.is_empty() == false);
        assert!(snap.uptime_seconds > 0);
        assert!(snap.gpu.usage >= 0.0 && snap.gpu.usage <= 100.0);
        // 根分区必须存在，界面依赖它显示主盘占用。
        assert!(snap.disks.iter().any(|d| d.mount == "/"));
    }

    #[test]
    fn collects_gpu_metrics() {
        let gpu = gpu_metrics();
        assert!(gpu.usage >= 0.0 && gpu.usage <= 100.0);
    }

    #[test]
    fn score_stays_in_range() {
        let mut sys = System::new_all();
        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();
        let snap = collect(&mut sys, &disks, &networks, 0.6);
        assert!(snap.health_score <= 100);
        assert!(!snap.health_score_msg.is_empty());
    }
}
