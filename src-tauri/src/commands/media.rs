#[cfg(target_os = "windows")]
use tauri::State;

#[cfg(target_os = "windows")]
use crate::system_media::SystemMediaManager;

/// 返回进程级 Windows 全局系统媒体管理器是否初始化成功。
#[cfg(target_os = "windows")]
#[tauri::command]
pub fn is_system_media_manager_initialized(state: State<'_, SystemMediaManager>) -> bool {
    state.is_initialized()
}

/// 非 Windows 平台没有可用的 Windows 全局系统媒体管理器。
#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn is_system_media_manager_initialized() -> bool {
    false
}
