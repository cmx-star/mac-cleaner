// Windows 下不生成控制台窗口；macOS 与 Linux 走普通入口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    mole_desktop_lib::run()
}
