//! fanctl — 风扇控制助手。
//!
//! 独立二进制，打包进 App Resources 后由用户设置 setuid root：
//!   sudo chown root:wheel fanctl && sudo chmod 4755 fanctl
//!
//! 用法（与 kuzen-so/mac-status 的 fanctl 一致）：
//!   fanctl --status        打印每个风扇的 target/actual
//!   fanctl --set <rpm>     手动设定全部风扇目标转速（自动进入强制模式）
//!   fanctl --auto          恢复系统自动控制
//!
//! 非 root 运行时 SMC 写入会失败，助手以非零码退出，不静默假装成功。

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("usage: fanctl --status | --set <rpm> | --auto");
        std::process::exit(64);
    }

    // 风扇数量：读不到时按 1 处理（绝大多数 Mac 至少有一个风扇）。
    let count = talpa_desktop_lib::smc::fan_count().max(1);

    match args[1].as_str() {
        "--status" => {
            for i in 0..count {
                let i_char = (b'0' + i as u8) as char;
                let target = talpa_desktop_lib::smc::read(&format!("F{i_char}Tg")).unwrap_or(-1.0);
                let actual = talpa_desktop_lib::smc::read(&format!("F{i_char}Ac")).unwrap_or(-1.0);
                println!("fan{i}: target={} actual={}", target as i32, actual as i32);
            }
        }
        "--set" => {
            let Some(rpm_str) = args.get(2) else {
                eprintln!("usage: fanctl --set <rpm>");
                std::process::exit(64);
            };
            let Ok(rpm) = rpm_str.parse::<f64>() else {
                eprintln!("无效转速：{rpm_str}");
                std::process::exit(64);
            };
            if !(0.0..=8000.0).contains(&rpm) {
                eprintln!("转速超出范围（0-8000）：{rpm}");
                std::process::exit(64);
            }
            if !set_all_fans(count, rpm) {
                eprintln!("SMC 写入失败（需要 root：sudo chown root:wheel fanctl && sudo chmod 4755 fanctl）");
                std::process::exit(2);
            }
        }
        "--auto" => {
            if !auto_all_fans(count) {
                eprintln!("SMC 写入失败（需要 root）");
                std::process::exit(2);
            }
        }
        other => {
            eprintln!("未知参数：{other}");
            eprintln!("usage: fanctl --status | --set <rpm> | --auto");
            std::process::exit(64);
        }
    }
}

/// 设定全部风扇：先写 `F{i}Md=1` 进入强制模式（否则系统持续覆写目标值），
/// 再写 `F{i}Tg` 目标转速。
fn set_all_fans(count: u32, rpm: f64) -> bool {
    let mut ok = true;
    for i in 0..count {
        let i_char = (b'0' + i as u8) as char;
        if talpa_desktop_lib::smc::write(&format!("F{i_char}Md"), 1.0).is_err() {
            ok = false;
        }
        if talpa_desktop_lib::smc::write(&format!("F{i_char}Tg"), rpm).is_err() {
            ok = false;
        }
    }
    ok
}

/// 恢复自动控制：`F{i}Md = 0`。
fn auto_all_fans(count: u32) -> bool {
    let mut ok = true;
    for i in 0..count {
        let i_char = (b'0' + i as u8) as char;
        if talpa_desktop_lib::smc::write(&format!("F{i_char}Md"), 0.0).is_err() {
            ok = false;
        }
    }
    ok
}
