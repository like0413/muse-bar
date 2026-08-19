use tauri::{AppHandle, State};

use crate::media_activity::{MediaSessionActivity, SelectedMediaSession};
use crate::system_media::{
    CurrentMediaMetadata, CurrentPlaybackCapabilities, CurrentPlaybackStatus, CurrentTimeline,
    MediaSessionIdentity, MediaSnapshot, SystemMediaManager,
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

/// 返回全部媒体会话的原始来源标识和播放器分类。
#[tauri::command]
pub fn get_media_session_identities(
    state: State<'_, SystemMediaManager>,
) -> Result<Vec<MediaSessionIdentity>, String> {
    state.session_identities()
}

/// 返回全部会话最近一次有效活动的时间、顺序和原因。
#[tauri::command]
pub fn get_media_session_activities(
    state: State<'_, SystemMediaManager>,
) -> Result<Vec<MediaSessionActivity>, String> {
    state.session_activities()
}

/// 根据最近有效活动重新选择 Bar 观察的会话。
#[tauri::command]
pub fn refresh_selected_media_session(
    app: AppHandle,
    state: State<'_, SystemMediaManager>,
) -> Result<Option<SelectedMediaSession>, String> {
    state.refresh_selected_media_session(&app)
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

/// 返回当前会话各项数据组成的统一媒体快照。
#[tauri::command]
pub fn get_current_media_snapshot(
    state: State<'_, SystemMediaManager>,
) -> Result<Option<MediaSnapshot>, String> {
    state.current_media_snapshot()
}
