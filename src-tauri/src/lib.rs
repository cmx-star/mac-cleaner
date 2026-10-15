//! Talpa / Mole 桌面版后端。
//!
//! 全量核心能力（状态监控、磁盘扫描、应用卸载、缓存清理、优化维护等）
//! 均使用纯 Rust 原生实现，不依赖外部 CLI。

pub mod apps;
pub mod clean;
pub mod cli;
pub mod commands;
pub mod history;
pub mod safety;
pub mod scan;
pub mod smc;
pub mod startup;
pub mod status;
pub mod trash;

use commands::Collector;

/// 构建并运行 Tauri 应用。
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Collector::default())
        .invoke_handler(tauri::generate_handler![
            commands::scan_directory,
            commands::extension_breakdown,
            commands::cleanup_preview,
            commands::system_status,
            commands::list_apps,
            commands::app_data,
            commands::read_history,
            commands::cli_status,
            commands::run_clean,
            commands::run_uninstall,
            commands::run_optimize,
            commands::read_whitelist,
            commands::home_dir,
            commands::startup_items,
            commands::set_startup_item,
            commands::open_login_items_settings,
            commands::classify_path,
            commands::fan_status,
            commands::set_fan,
            commands::app_icon,
            commands::top_processes,
            commands::trash_paths,
            commands::toggle_low_latency,
            commands::kill_process,
            commands::reveal_in_finder,
            commands::process_detail,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Talpa 桌面版失败");
}
