//! Mole 桌面版后端。
//!
//! 分工：只读能力（扫描、状态、应用列表、历史）在本进程内实现；
//! 破坏性操作（清理、卸载、优化）转发给 Mole CLI，由它的安全层执行。

pub mod apps;
pub mod cli;
pub mod commands;
pub mod history;
pub mod safety;
pub mod scan;
pub mod smc;
pub mod status;

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
            commands::classify_path,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Mole 桌面版失败");
}
