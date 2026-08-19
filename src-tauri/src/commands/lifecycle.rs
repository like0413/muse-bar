use tauri::AppHandle;

/// 允许 Bar 前端复用 Rust 的唯一设置窗口创建流程。
#[tauri::command]
pub fn open_settings_window(app: AppHandle) -> Result<(), String> {
    crate::app_lifecycle::open_settings_window(&app)
}

/// 设置前端完成首次数据读取和渲染后，请求显示原生窗口。
#[tauri::command]
pub fn show_ready_settings_window(app: AppHandle) -> Result<(), String> {
    crate::app_lifecycle::show_ready_settings_window(&app)
}
