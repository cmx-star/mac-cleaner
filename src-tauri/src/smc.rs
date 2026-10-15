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

#[link(name = "IOFramework", kind = "framework")]
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

/// SMCParamStruct。总大小 80 字节，手工排布保证与内核接口严格一致
/// （Swift 侧用 MemoryLayout 断言 size==80、bytes 偏移 48 验证过）：
/// key(4) + data8(1，偏移 4) + pad(3) + data32(4，偏移 8) + vers/limit 区(8..40)
/// + keyInfo(40..52) + result/status(52..54) + bytes(48..80，与 vers/limit 区重叠是
/// 内核接口本身的设计：bytes 是读结果的通用载荷区)。
///
/// 为避免对齐陷阱，整体用 [u8; 80] 存储并按偏移读写。
#[derive(Clone, Copy)]
struct SmcParamStruct {
    raw: [u8; 80],
}


impl SmcParamStruct {
    fn zeroed() -> Self {
        Self { raw: [0; 80] }
    }

    fn set_key(&mut self, key: &str) {
        let b = key.as_bytes();
        self.raw[0..4].copy_from_slice(b);
    }

    /// 子命令字节，偏移 4。
    fn data8(&self) -> u8 {
        self.raw[4]
    }

    fn set_data8(&mut self, v: u8) {
        self.raw[4] = v;
    }

    /// keyInfo 区，偏移 40..52：dataSize u32 | dataType u32 | dataAttributes u8 + pad(3)。
    fn key_info(&self) -> [u8; 12] {
        self.raw[40..52].try_into().unwrap()
    }

    fn set_key_info(&mut self, info: [u8; 12]) {
        self.raw[40..52].copy_from_slice(&info);
    }

    fn key_type(&self) -> u32 {
        u32::from_be_bytes(self.raw[44..48].try_into().unwrap())
    }

    fn key_data_size(&self) -> usize {
        u32::from_be_bytes(self.raw[40..44].try_into().unwrap()) as usize
    }

    fn result(&self) -> u8 {
        self.raw[52]
    }

    /// 数据载荷区，偏移 48..80。
    fn bytes(&self) -> &[u8; 32] {
        self.raw[48..80].try_into().unwrap()
    }

    fn set_bytes(&mut self, data: &[u8]) {
        self.raw[48..48 + data.len()].copy_from_slice(data);
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
    if output.result() != 0 {
        return Err(format!("SMC 返回错误码 {}", output.result()));
    }
    Ok(output)
}

/// GetKeyInfo：拿键的类型与大小。
fn key_info(key: &str) -> Result<(u32, usize), String> {
    let mut input = SmcParamStruct::zeroed();
    input.set_key(key);
    input.data8 = SMC_GET_KEY_INFO;

    let out = smc_call(&input)?;
    Ok((out.key_type(), out.key_data_size()))
}

/// 读原始键值，返回 (4 字符类型码, 数据字节)。
pub fn read_key_raw(key: &str) -> Result<(String, Vec<u8>), String> {
    if key.as_bytes().len() != 4 {
        return Err(format!("SMC 键必须是 4 字节：{key}"));
    }

    let (data_type, data_size) = key_info(key)?;
    let data_size = data_size.min(32);

    let mut input = SmcParamStruct::zeroed();
    input.set_key(key);
    input.data8 = SMC_READ_KEY;

    let out = smc_call(&input)?;
    let type_str: String = data_type.to_be_bytes().iter().map(|b| *b as char).collect();
    Ok((type_str, out.bytes[..data_size].to_vec()))
}

/// 读取数值，按类型自动解码。
pub fn read(key: &str) -> Option<f64> {
    let (type_str, data) = read_key_raw(key).ok()?;
    decode(&type_str, &data)
}

/// 按类型码解码。
fn decode(type_str: &str, data: &[u8]) -> Option<f64> {
    match type_str {
        // IEEE 754 单精度。
        "flt " => {
            if data.len() < 4 {
                return None;
            }
            Some(f32::from_be_bytes([data[0], data[1], data[2], data[3]]) as f64)
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

/// 写数值。写入时 keyInfo 必须带上真实的类型与大小（与 mac-status 一致）。
/// 写 SMC 需要 root；普通进程会失败，由 fanctl 助手（setuid root）执行。
pub fn write(key: &str, value: f64) -> Result<(), String> {
    if key.as_bytes().len() != 4 {
        return Err(format!("SMC 键必须是 4 字节：{key}"));
    }

    let (data_type, data_size) = key_info(key)?;

    let bytes: Vec<u8> = match data_type {
        0x666C7420 => f32::from(value as f32).to_be_bytes().to_vec(),          // "flt "
        0x75693136 => (value as u16).to_be_bytes().to_vec(),                   // "ui16"
        0x75693820 => vec![value as u8],                                       // "ui8 "
        0x66706532 => ((value * 4.0).round() as u16).to_be_bytes().to_vec(),   // "fpe2"
        0x73703738 => ((value * 256.0).round() as u16).to_be_bytes().to_vec(), // "sp78"
        other => {
            let s: String = other.to_be_bytes().iter().map(|b| *b as char).collect();
            return Err(format!("SMC 键类型 {s} 不支持写入"));
        }
    };

    let mut input = SmcParamStruct::zeroed();
    input.set_key(key);
    input.data8 = SMC_WRITE_KEY;
    let mut info = [0u8; 12];
    info[..4].copy_from_slice(&(data_size as u32).to_be_bytes());
    info[4..8].copy_from_slice(&data_type.to_be_bytes());
    input.set_key_info(info);
    input.bytes[..bytes.len()].copy_from_slice(&bytes);

    smc_call(&input)?;
    Ok(())
}

/// 温度传感器键候选。Apple Silicon 上 `TC0P` 是 CPU 近似温度。
const CPU_TEMP_KEYS: &[&str] = &["TC0P", "TC0D", "Tp01", "Tp09"];
const GPU_TEMP_KEYS: &[&str] = &["TG0P", "TG0D", "Tg05"];

fn plausible_temp(v: f64) -> bool {
    v > 0.0 && v < 150.0
}

/// 读取 CPU 温度（摄氏度）。多键依次尝试。
pub fn cpu_temp() -> Option<f64> {
    CPU_TEMP_KEYS
        .iter()
        .find_map(|k| read(k).filter(|v| plausible_temp(*v)))
}

/// 读取 GPU 温度（摄氏度）。
pub fn gpu_temp() -> Option<f64> {
    GPU_TEMP_KEYS
        .iter()
        .find_map(|k| read(k).filter(|v| plausible_temp(*v)))
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
#[derive(Debug, Clone, Copy, serde::Serialize)]
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
        assert_eq!(decode("flt ", &v.to_be_bytes()), Some(45.5));
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
        if let Some(t) = cpu_temp() {
            assert!(plausible_temp(t), "CPU 温度应在合理范围：{t}");
        }
        for fan in fans() {
            assert!(fan.rpm > 0.0, "风扇转速应为正：{}", fan.rpm);
            if fan.max_rpm > 0.0 {
                assert!(fan.rpm <= fan.max_rpm * 1.1, "转速不应超过最大值");
            }
        }
    }
}