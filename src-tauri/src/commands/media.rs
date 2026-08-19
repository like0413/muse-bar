use tauri::State;

use crate::system_media::{
    CurrentMediaMetadata, CurrentPlaybackCapabilities, CurrentPlaybackStatus, CurrentTimeline,
    SystemMediaManager,
};

/// 返回进程级 Windows 全局系统媒体管理器是否初始化成功。
#[tauri::command]
pub fn is_system_media_manager_initialized(state: State<'_, SystemMediaManager>) -> bool {
    state.is_initialized()
}

/// 返回当前所有 Windows 系统媒体会话的 Source App ID。
#[tauri::command]
pub fn get_media_session_source_app_ids(
    state: State<'_, SystemMediaManager>,
) -> Result<Vec<String>, String> {
    state.source_app_ids()
}

/// 从后台缓存返回 Windows 当前媒体会话的标题、歌手和封面。
#[tauri::command]
pub fn get_current_media_metadata(
    state: State<'_, SystemMediaManager>,
) -> Result<Option<CurrentMediaMetadata>, String> {
    state.current_media_metadata()
}

/// 返回 Windows 当前媒体会话的播放状态。
#[tauri::command]
pub fn get_current_playback_status(
    state: State<'_, SystemMediaManager>,
) -> Result<Option<CurrentPlaybackStatus>, String> {
    state.current_playback_status()
}

/// 返回 Windows 当前媒体会话声明的控制能力。
#[tauri::command]
pub fn get_current_playback_capabilities(
    state: State<'_, SystemMediaManager>,
) -> Result<Option<CurrentPlaybackCapabilities>, String> {
    state.current_playback_capabilities()
}

/// 返回 Windows 当前媒体会话上报的有效时间轴。
#[tauri::command]
pub fn get_current_timeline(
    state: State<'_, SystemMediaManager>,
) -> Result<Option<CurrentTimeline>, String> {
    state.current_timeline()
}
