use tauri::{AppHandle, State};

use crate::media_activity::{MediaSessionActivity, SelectedMediaSession};
use crate::media_control::{ControlAction, MediaControlError};
use crate::system_media::{MediaSessionIdentity, MediaSnapshot, SystemMediaManager};

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

/// 对 Bar 当前选择的媒体会话执行播放、暂停、切歌或进度跳转。
#[tauri::command]
pub fn control_media(
    state: State<'_, SystemMediaManager>,
    action: ControlAction,
) -> Result<(), MediaControlError> {
    state.control_media(action)
}

/// 返回当前会话各项数据组成的统一媒体快照。
#[tauri::command]
pub fn get_current_media_snapshot(
    state: State<'_, SystemMediaManager>,
) -> Result<Option<MediaSnapshot>, String> {
    state.current_media_snapshot()
}
