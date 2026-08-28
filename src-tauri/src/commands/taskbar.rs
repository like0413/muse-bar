use crate::taskbar::TaskbarMonitor;

/// 返回当前具有 Windows 任务栏的显示器，供设置页选择 Bar 的宿主。
#[tauri::command]
pub fn get_taskbar_monitors() -> Result<Vec<TaskbarMonitor>, String> {
    crate::taskbar::list_taskbar_monitors()
}
