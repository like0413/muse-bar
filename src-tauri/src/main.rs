// Windows 发布版不显示额外的控制台窗口，请勿删除。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// 进入 Muse Bar 共享的 Tauri 运行时。
fn main() {
    muse_bar_lib::run();
}
