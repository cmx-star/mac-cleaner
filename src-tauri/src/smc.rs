//! SMC（System Management Controller）传感器读取与风扇控制。
//!
//! 协议参考 kuzen-so/mac-status 的 SMC.swift：通过 IOKit 的 AppleSMC 用户客户端，
//! selector 2（kSMCHandleYPCEvent）收发 80 字节的 SMCParamStruct；读键前先
//! GetKeyInfo（selector 9）拿类型与大小。
//!
//! 数据类型：`flt ` IEEE 浮点、`fpe2` 定点 14.2（除 4）、`sp78` 定点 8.8（除 256）、
//! `ui8 `/`ui16`/`ui32` 无符号整数。
//!
//! 权限：读取在 Apple Silicon 上无特权可用；写入（风扇控制）需要 root，
//! 由打包的 fanctl 助手（setuid root）完成，见 `set_fan_mode`。

use std::ffi::c_void;
use std::sync::Mutex;

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> u32;
    fn IOServiceMatching(name: *const i8) -> *mut c_void;
    fn IOServiceOpen(service: u32, owning_task: u32, conn_type: u32, connect: *mut u32) -> i32;
    fn IOObjectRelease(obj: u32) -> i32;
    fn IOServiceClose(connect: u32) -> i32;
    fn IOConnectCallMethod(
        connect: u32,
        selector: u32,
        input_scalar: *const u64,
        scalar_cnt: u32,
        input_struct: *const c_void,
        input_struct_cnt: usize,
        output_scalar: *mut u64,
        output_scalar_cnt: *mut u32,
        output_struct: *mut c_void,
        output_struct_cnt: *mut usize,
    ) -> i32;
    fn mach_task_self() -> u32;
}

/// 选择器 2：AppleSMC 的事件驱动调用。
const SMC_SELECTOR: u32 = 2;
/// 子命令：GetKeyInfo。
const SMC_GET_KEY_INFO: u8 = 9;
/// 子命令：ReadKey。
const SMC_READ_KEY: u8 = 5;
/// 子命令：WriteKey。
const SMC_WRITE_KEY: u8 = 6;
/// 结构体大小必须严格等于 80。
const SMC_STRUCT_SIZE: usize = 80;

// ---- 字段偏移（在 Apple M4 / macOS 15.8 上实测确定）----
//
// 全部 80 个偏移里，只有 data8=42 的调用会被内核接受（其余返回
// kIOReturnNotPrivileged -536870206），据此定出下列偏移。
const OFF_KEY: usize = 0; // key，小端 u32
const OFF_KEYINFO_SIZE: usize = 28; // keyInfo.dataSize，小端 u32
const OFF_KEYINFO_TYPE: usize = 32; // keyInfo.dataType，小端 u32
const OFF_RESULT: usize = 36; // result 字节
const OFF_DATA8: usize = 42; // 子命令字节
const OFF_BYTES: usize = 48; // 数据载荷，32 字节

/// SMCParamStruct，按字节排布以精确控制偏移。
///
/// 两个容易踩的坑（本项目都踩过，导致 Apple Silicon 上一直读不到数据）：
/// 1. `key` 由内核按**小端 u32** 解释，不能直接写 4 个 ASCII 字节。
///    `"F0Ac"` 必须写成 `63 41 30 46`。
/// 2. `result` 在偏移 36，不在 40 或 52。读错位置会把成功调用误判成失败。
#[derive(Clone, Copy)]
struct SmcParamStruct {
    raw: [u8; 80],
}

impl SmcParamStruct {
    fn zeroed() -> Self {
        Self { raw: [0; 80] }
    }

    /// 写入 key。按小端 u32 排列，与内核的 FourCharCode 解释一致。
    fn set_key(&mut self, key: &str) {
        let b = key.as_bytes();
        self.raw[OFF_KEY] = b[3];
        self.raw[OFF_KEY + 1] = b[2];
        self.raw[OFF_KEY + 2] = b[1];
        self.raw[OFF_KEY + 3] = b[0];
    }

    /// 子命令字节。
    fn set_data8(&mut self, v: u8) {
        self.raw[OFF_DATA8] = v;
    }

    /// keyInfo.dataSize（小端 u32）。
    fn key_data_size(&self) -> usize {
        let p = &self.raw[OFF_KEYINFO_SIZE..OFF_KEYINFO_SIZE + 4];
        u32::from_le_bytes(p.try_into().unwrap()) as usize
    }

    fn set_key_data_size(&mut self, size: u32) {
        self.raw[OFF_KEYINFO_SIZE..OFF_KEYINFO_SIZE + 4].copy_from_slice(&size.to_le_bytes());
    }

    /// keyInfo.dataType，转回可读的 4 字符类型码（如 `flt ` / `sp78` / `ui8 `）。
    fn key_data_type(&self) -> String {
        let p = &self.raw[OFF_KEYINFO_TYPE..OFF_KEYINFO_TYPE + 4];
        p.iter().rev().map(|b| *b as char).collect()
    }

    fn result(&self) -> u8 {
        self.raw[OFF_RESULT]
    }

    /// 数据载荷区（偏移 48..80）。
    fn bytes(&self) -> &[u8] {
        &self.raw[OFF_BYTES..OFF_BYTES + 32]
    }

    fn set_bytes(&mut self, data: &[u8]) {
        let n = data.len().min(32);
        self.raw[OFF_BYTES..OFF_BYTES + n].copy_from_slice(&data[..n]);
    }
}

const _: () = assert!(std::mem::size_of::<SmcParamStruct>() == SMC_STRUCT_SIZE);

/// 打开的 SMC 连接句柄缓存。-1 未初始化，-2 初始化失败。
static CONN: Mutex<i32> = Mutex::new(-1);

/// 打开 AppleSMC 连接。
fn open_smc() -> Result<u32, String> {
    unsafe {
        let matching = IOServiceMatching(b"AppleSMC\0".as_ptr() as *const i8);
        if matching.is_null() {
            return Err("无法构造 AppleSMC 匹配字典".into());
        }
        let service = IOServiceGetMatchingService(0, matching);
        if service == 0 {
            return Err("未找到 AppleSMC 服务".into());
        }
        let mut connect: u32 = 0;
        let kr = IOServiceOpen(service, mach_task_self(), 0, &mut connect);
        // GetMatchingService 消费了一个引用，用完释放。
        IOObjectRelease(service);
        if kr != 0 {
            return Err(format!("IOServiceOpen 失败（kern_return {kr}）"));
        }
        Ok(connect)
    }
}

/// 发起一次 SMC 调用。
fn smc_call(input: &SmcParamStruct) -> Result<SmcParamStruct, String> {
    let mut cached = CONN.lock().map_err(|_| "SMC 连接锁被占用".to_string())?;

    if *cached == -1 {
        match open_smc() {
            Ok(c) => *cached = c as i32,
            Err(e) => {
                *cached = -2;
                return Err(e);
            }
        }
    }
    if *cached == -2 {
        return Err("SMC 连接不可用".into());
    }
    let conn = *cached as u32;

    let mut output = SmcParamStruct::zeroed();
    let mut output_size = SMC_STRUCT_SIZE;

    let kr = unsafe {
        IOConnectCallMethod(
            conn,
            SMC_SELECTOR,
            std::ptr::null(),
            0,
            input as *const _ as *const c_void,
            SMC_STRUCT_SIZE,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut output as *mut _ as *mut c_void,
            &mut output_size,
        )
    };
    if kr != 0 {
        return Err(format!("SMC 调用失败（kern_return {kr}）"));
    }
    // 不在这里检查 result：实测读 keyInfo 时该字节为 0x84 但调用实际成功，
    // 成功与否由调用方按 keyInfo.dataSize 判断。
    Ok(output)
}

/// GetKeyInfo：拿键的类型与大小。
fn key_info(key: &str) -> Result<(String, usize), String> {
    let mut input = SmcParamStruct::zeroed();
    input.set_key(key);
    input.set_data8(SMC_GET_KEY_INFO);

    let out = smc_call(&input)?;

    let size = out.key_data_size();
    // 键不存在时 size 为 0。内核在找不到键时不一定填 result，以 size 为准更可靠。
    if size == 0 || size > 32 {
        return Err(format!("SMC 键不存在或大小异常：{key}"));
    }
    Ok((out.key_data_type(), size))
}

/// 读原始键值，返回 (4 字符类型码, 数据字节)。
pub fn read_key_raw(key: &str) -> Result<(String, Vec<u8>), String> {
    if key.as_bytes().len() != 4 {
        return Err(format!("SMC 键必须是 4 字节：{key}"));
    }

    let (type_str, data_size) = key_info(key)?;

    let mut input = SmcParamStruct::zeroed();
    input.set_key(key);
    input.set_data8(SMC_READ_KEY);
    // 读值时要把 keyInfo.dataSize 带回去，否则内核不知道要拷多少字节。
    input.set_key_data_size(data_size as u32);

    let out = smc_call(&input)?;
    Ok((type_str, out.bytes()[..data_size].to_vec()))
}

/// 读取数值，按类型自动解码。
pub fn read(key: &str) -> Option<f64> {
    let (type_str, data) = read_key_raw(key).ok()?;
    decode(&type_str, &data)
}

/// 按类型码解码。
fn decode(type_str: &str, data: &[u8]) -> Option<f64> {
    match type_str {
        // IEEE 754 单精度。实测字节序为【小端】：`00 d0 01 45` = 2073.0 RPM。
        // （Apple Silicon 上 SMC 的 flt 与 key 一样按小端存取。）
        "flt " => {
            if data.len() < 4 {
                return None;
            }
            Some(f32::from_le_bytes([data[0], data[1], data[2], data[3]]) as f64)
        }
        // 定点 14.2：大端整数除以 4（Intel 风扇转速）。
        "fpe2" => {
            if data.len() < 2 {
                return None;
            }
            Some((((data[0] as u32) << 8) | data[1] as u32) as f64 / 4.0)
        }
        // 定点 8.8：大端整数除以 256（温度）。
        "sp78" => {
            if data.len() < 2 {
                return None;
            }
            Some((((data[0] as u32) << 8) | data[1] as u32) as f64 / 256.0)
        }
        "ui8 " => data.first().map(|b| *b as f64),
        "ui16" => {
            if data.len() < 2 {
                return None;
            }
            Some(u16::from_be_bytes([data[0], data[1]]) as f64)
        }
        "ui32" => {
            if data.len() < 4 {
                return None;
            }
            Some(u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as f64)
        }
        _ => None,
    }
}

/// 写数值。写入时 keyInfo 必须带上真实的类型与大小。
/// 写 SMC 需要 root；普通进程会失败，由 fanctl 助手（setuid root）执行。
pub fn write(key: &str, value: f64) -> Result<(), String> {
    if key.as_bytes().len() != 4 {
        return Err(format!("SMC 键必须是 4 字节：{key}"));
    }

    let (type_str, data_size) = key_info(key)?;

    let bytes: Vec<u8> = match type_str.as_str() {
        "flt " => (value as f32).to_le_bytes().to_vec(),
        "ui16" => (value as u16).to_be_bytes().to_vec(),
        "ui8 " => vec![value as u8],
        "fpe2" => ((value * 4.0).round() as u16).to_be_bytes().to_vec(),
        "sp78" => ((value * 256.0).round() as u16).to_be_bytes().to_vec(),
        other => return Err(format!("SMC 键类型 {other} 不支持写入")),
    };

    let mut input = SmcParamStruct::zeroed();
    input.set_key(key);
    input.set_data8(SMC_WRITE_KEY);
    input.set_key_data_size(data_size as u32);
    input.set_bytes(&bytes);

    smc_call(&input)?;
    Ok(())
}

/// 温度传感器键候选。Apple Silicon 上 `TC0P` 是 CPU 近似温度。
/// CPU 温度键候选。
///
/// Apple Silicon（M 系列）用 `Tp0x` 系列，实测 M4 上存在 Tp01/Tp02/Tp04..Tp0f
/// 共 13 个核心传感器；Intel 机型用 `TC0P`/`TC0D`。两套都列上，读到哪个用哪个。
const CPU_TEMP_KEYS: &[&str] = &[
    // Apple Silicon 核心传感器
    "Tp01", "Tp02", "Tp04", "Tp05", "Tp06", "Tp08", "Tp09", "Tp0a", "Tp0b", "Tp0c", "Tp0d",
    "Tp0e", "Tp0f",
    // Intel 机型
    "TC0P", "TC0D", "TC0E", "TC0F",
];

/// GPU 温度键候选。Apple Silicon 用 `Tg0x`，Intel 用 `TG0P`/`TG0D`。
const GPU_TEMP_KEYS: &[&str] = &["Tg0D", "Tg0L", "Tg0d", "Tg0k", "Tg0T", "TG0P", "TG0D"];

fn plausible_temp(v: f64) -> bool {
    v > 0.0 && v < 150.0
}

/// 读取 CPU 温度（摄氏度）：取全部可用核心传感器里的最高值。
///
/// 取最高值而不是平均值，因为用户关心的是「会不会过热」，
/// 单个核心冲到 100°C 时平均值会被其他核心稀释掉。
/// 读不到任何核心传感器时退回到「第一个可读的候选键」。
pub fn cpu_temp() -> Option<f64> {
    let temps: Vec<f64> = CPU_TEMP_KEYS
        .iter()
        .filter_map(|k| read(k))
        .filter(|v| plausible_temp(*v))
        .collect();

    temps.iter().cloned().fold(None, |acc: Option<f64>, v| {
        Some(match acc {
            Some(m) if m > v => m,
            _ => v,
        })
    })
}

/// 读取 GPU 温度（摄氏度）：同样取最高值。
pub fn gpu_temp() -> Option<f64> {
    GPU_TEMP_KEYS
        .iter()
        .filter_map(|k| read(k))
        .filter(|v| plausible_temp(*v))
        .fold(None, |acc: Option<f64>, v| {
            Some(match acc {
                Some(m) if m > v => m,
                _ => v,
            })
        })
}

/// 风扇信息。
#[derive(Debug, Clone, serde::Serialize)]
pub struct Fan {
    /// 当前转速。
    pub rpm: f64,
    /// 目标转速。
    pub target_rpm: f64,
    /// 最低转速。
    pub min_rpm: f64,
    /// 最高转速。
    pub max_rpm: f64,
    /// 手动模式标记（`F{i}Md` = 1）。
    pub forced: bool,
    /// 风扇编号。
    pub index: u32,
}

/// 风扇数量（`FNum`）。
pub fn fan_count() -> u32 {
    read("FNum").map(|v| v as u32).unwrap_or(0)
}

/// 读取指定风扇的完整信息。
pub fn fan(index: u32) -> Option<Fan> {
    let i = (b'0' + index as u8) as char;
    let rpm = read(&format!("F{i}Ac"))?;
    let target = read(&format!("F{i}Tg")).unwrap_or(0.0);
    let min = read(&format!("F{i}Mn")).unwrap_or(0.0);
    let max = read(&format!("F{i}Mx")).unwrap_or(0.0);
    let mode = read(&format!("F{i}Md")).unwrap_or(0.0);
    Some(Fan {
        rpm,
        target_rpm: target,
        min_rpm: min,
        max_rpm: max,
        forced: mode >= 1.0,
        index,
    })
}

/// 读取全部风扇。
pub fn fans() -> Vec<Fan> {
    (0..fan_count()).filter_map(fan).collect()
}

/// 风扇控制模式。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FanMode {
    /// 恢复系统自动控制（写 `F{i}Md = 0`）。
    Auto,
    /// 强制模式 + 固定目标转速（写 `F{i}Md = 1` 再写 `F{i}Tg`）。
    Manual(u32),
}

/// 设置风扇模式。走打包的 fanctl 助手（setuid root）执行，与 mac-status 方案一致。
/// 助手未安装时返回带指引的错误，不静默失败。
pub async fn set_fan_mode(mode: FanMode) -> Result<(), String> {
    let helper = helper_path().ok_or_else(|| {
        "风扇控制助手（fanctl）未安装。安装方式：\
         sudo chown root:wheel fanctl && sudo chmod 4755 fanctl"
    })?;

    let output = match mode {
        FanMode::Auto => {
            tokio::process::Command::new(&helper).arg("--auto").output().await
        }
        FanMode::Manual(rpm) => {
            // 防御：限制在合法范围（mac-status 用 0..=8000），不把任意值写进 SMC。
            let rpm = rpm.clamp(0, 8000);
            tokio::process::Command::new(&helper)
                .arg("--set")
                .arg(rpm.to_string())
                .output()
                .await
        }
    };

    let output = output.map_err(|e| format!("执行 fanctl 失败：{e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "fanctl 退出码 {:?}：{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// 定位 fanctl 助手：App Resources 优先，开发模式找同目录。
fn helper_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    // .app/Contents/MacOS/<exe> → ../Resources/fanctl。
    let resources = exe
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("Resources/fanctl"));
    if let Some(p) = resources {
        if p.exists() {
            return Some(p);
        }
    }
    let sibling = exe.parent().map(|p| p.join("fanctl"));
    if let Some(p) = sibling {
        if p.exists() {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_size_is_80() {
        assert_eq!(std::mem::size_of::<SmcParamStruct>(), 80);
    }

    #[test]
    fn decodes_fpe2_fan_rpm() {
        // 2000 rpm → 2000*4 = 8000 = 0x1F40。
        assert_eq!(decode("fpe2", &[0x1F, 0x40]), Some(2000.0));
        assert_eq!(decode("fpe2", &[0x13, 0x88]), Some(1250.0));
    }

    #[test]
    fn decodes_sp78_temperature() {
        // 45.5°C → 45.5*256 = 11648 = 0x2D80。
        assert_eq!(decode("sp78", &[0x2D, 0x80]), Some(45.5));
        assert_eq!(decode("sp78", &[0x55, 0x00]), Some(85.0));
    }

    #[test]
    fn decodes_flt() {
        let v = 45.5f32;
        assert_eq!(decode("flt ", &v.to_le_bytes()), Some(45.5));
    }

    #[test]
    fn decodes_real_fan_sample() {
        // 实测样本：`00 d0 01 45` 是小端的 0x4501d000 ≈ 2077.0 RPM。
        assert_eq!(decode("flt ", &[0x00, 0xd0, 0x01, 0x45]), Some(2077.0));
    }

    #[test]
    fn decodes_uints() {
        assert_eq!(decode("ui8 ", &[42]), Some(42.0));
        assert_eq!(decode("ui16", &[0x1F, 0x40]), Some(8000.0));
        assert_eq!(decode("ui32", &[0, 0, 0x1F, 0x40]), Some(8000.0));
    }

    #[test]
    fn rejects_short_data() {
        assert_eq!(decode("fpe2", &[]), None);
        assert_eq!(decode("sp78", &[0x2D]), None);
        assert_eq!(decode("flt ", &[0, 0]), None);
    }

    #[test]
    fn rejects_unknown_types() {
        assert_eq!(decode("ch8*", &[1, 2]), None);
    }

    #[test]
    fn rejects_malformed_keys() {
        assert!(read_key_raw("TOOLONG").is_err());
        assert!(read_key_raw("").is_err());
        assert!(write("TOOLONG", 1.0).is_err());
    }

    #[test]
    fn live_smc_read_works_or_fails_gracefully() {
        // 真实 mac：能读则值合理；读不到则优雅降级，不 panic。
        // 读不到键（非 Apple 硬件、键不存在）时返回 None，界面降级显示。
        if let Some(t) = cpu_temp() {
            assert!(plausible_temp(t), "CPU 温度应在合理范围：{t}");
        }
        if let Some(t) = gpu_temp() {
            assert!(plausible_temp(t), "GPU 温度应在合理范围：{t}");
        }
        for fan in fans() {
            assert!(fan.rpm > 0.0, "风扇转速应为正：{}", fan.rpm);
            if fan.max_rpm > 0.0 {
                // 手动模式下目标值可能略超标称上限，留 10% 余量。
                assert!(
                    fan.rpm <= fan.max_rpm * 1.1,
                    "转速 {} 不应超过最大值 {} 的 110%",
                    fan.rpm,
                    fan.max_rpm
                );
            }
        }
    }

    #[test]
    fn cpu_temp_takes_the_hottest_sensor() {
        // Apple Silicon 上 CPU 有多个核心传感器，cpu_temp() 取最高值：
        // 单个核心逼近上限才是用户要看的信号，平均值会把它稀释掉。
        // 这里用真实机器验证：读到的值必须落在任一核心传感器的区间内。
        let Some(hottest) = cpu_temp() else {
            return; // 非 mac 或读不到时跳过
        };
        let all: Vec<f64> = CPU_TEMP_KEYS
            .iter()
            .filter_map(|k| read(k))
            .filter(|v| plausible_temp(*v))
            .collect();
        if all.is_empty() {
            return;
        }
        let max = all.iter().cloned().fold(f64::MIN, f64::max);
        let min = all.iter().cloned().fold(f64::MAX, f64::min);
        assert!(
            (hottest - max).abs() < 0.001,
            "cpu_temp() 应返回最高值 {max}，实际 {hottest}"
        );
        assert!(hottest >= min, "返回值不应低于最低传感器 {min}");
    }

    #[test]
    fn reports_which_keys_exist_on_this_machine() {
        // 诊断用：列出关键键的可读性，便于在不同机型上排查。
        for key in ["FNum", "F0Ac", "F0Tg", "F0Md", "F0Mn", "F0Mx", "TC0P", "TG0P"] {
            let result = read_key_raw(key);
            match result {
                Ok((t, d)) => println!("{key}: type={t} bytes={:02x?} value={:?}", d, read(key)),
                Err(e) => println!("{key}: 不可读（{e}）"),
            }
        }
    }
}