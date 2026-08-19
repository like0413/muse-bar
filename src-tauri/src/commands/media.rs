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

/// 返回当前所有 Windows 系统媒体会话的 Source App ID。
#[cfg(target_os = "windows")]
#[tauri::command]
pub fn get_media_session_source_app_ids(
    state: State<'_, SystemMediaManager>,
) -> Result<Vec<String>, String> {
    state.source_app_ids()
}

/// 非 Windows 平台没有可用的 Windows 全局系统媒体管理器。
#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn is_system_media_manager_initialized() -> bool {
    false
}

/// 非 Windows 平台返回明确的不支持错误。
#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn get_media_session_source_app_ids() -> Result<Vec<String>, String> {
    Err("系统媒体会话仅支持 Windows".to_owned())
}
