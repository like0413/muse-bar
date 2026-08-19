#[cfg(target_os = "windows")]
use tauri::State;

#[cfg(target_os = "windows")]
use crate::system_media::{
    CurrentMediaMetadata, CurrentPlaybackCapabilities, CurrentPlaybackStatus, SystemMediaManager,
};

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

/// 返回 Windows 当前媒体会话的 Source App ID、标题和歌手。
#[cfg(target_os = "windows")]
#[tauri::command]
pub fn get_current_media_metadata(
    state: State<'_, SystemMediaManager>,
) -> Result<Option<CurrentMediaMetadata>, String> {
    state.current_media_metadata()
}

/// 返回 Windows 当前媒体会话的播放状态。
#[cfg(target_os = "windows")]
#[tauri::command]
pub fn get_current_playback_status(
    state: State<'_, SystemMediaManager>,
) -> Result<Option<CurrentPlaybackStatus>, String> {
    state.current_playback_status()
}

/// 返回 Windows 当前媒体会话声明的控制能力。
#[cfg(target_os = "windows")]
#[tauri::command]
pub fn get_current_playback_capabilities(
    state: State<'_, SystemMediaManager>,
) -> Result<Option<CurrentPlaybackCapabilities>, String> {
    state.current_playback_capabilities()
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

/// 非 Windows 平台没有可读取的 Windows 当前媒体会话。
#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn get_current_media_metadata() -> Result<Option<()>, String> {
    Err("当前媒体元数据仅支持 Windows".to_owned())
}

/// 非 Windows 平台没有可读取的 Windows 当前播放状态。
#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn get_current_playback_status() -> Result<Option<()>, String> {
    Err("当前媒体播放状态仅支持 Windows".to_owned())
}

/// 非 Windows 平台没有可读取的 Windows 当前控制能力。
#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn get_current_playback_capabilities() -> Result<Option<()>, String> {
    Err("当前媒体控制能力仅支持 Windows".to_owned())
}
