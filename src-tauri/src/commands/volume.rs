use tauri::{AppHandle, Manager, State};

use crate::{
    media::SystemMediaManager,
    volume::{
        ApplicationVolumeAction, ApplicationVolumeError, ApplicationVolumeManager,
        ApplicationVolumeState,
    },
    volume_flyout::{VolumeFlyoutAnchor, VolumeFlyoutManager},
};

/// 读取当前媒体应用的会话音量；没有可匹配音频会话时返回空值。
#[tauri::command]
pub fn get_current_application_volume(
    app: AppHandle,
    media: State<'_, SystemMediaManager>,
    volume: State<'_, ApplicationVolumeManager>,
    expected_session_key: u64,
) -> Result<Option<ApplicationVolumeState>, ApplicationVolumeError> {
    let identity = media
        .current_volume_identity(&app, expected_session_key)
        .map_err(ApplicationVolumeError::media_identity)?;
    volume.query(identity)
}

/// 按音量按钮的屏幕位置懒创建并显示无焦点浮层。
#[tauri::command]
pub async fn show_application_volume_flyout(
    app: AppHandle,
    anchor: VolumeFlyoutAnchor,
    expected_session_key: u64,
    accent_color: String,
) -> Result<(), String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        worker_app.state::<VolumeFlyoutManager>().show(
            &worker_app,
            anchor,
            expected_session_key,
            accent_color,
        )
    })
    .await
    .map_err(|error| format!("音量浮层创建任务意外停止：{error}"))?
}

/// 音量浮层完成首次渲染后，应用尚未消费的显示请求。
#[tauri::command]
pub async fn show_ready_application_volume_flyout(app: AppHandle) -> Result<(), String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        worker_app
            .state::<VolumeFlyoutManager>()
            .mark_ready_and_show(&worker_app)
    })
    .await
    .map_err(|error| format!("音量浮层显示任务意外停止：{error}"))?
}

/// 隐藏音量浮层并取消尚未完成的显示请求。
#[tauri::command]
pub async fn hide_application_volume_flyout(app: AppHandle) -> Result<(), String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        worker_app.state::<VolumeFlyoutManager>().hide(&worker_app)
    })
    .await
    .map_err(|error| format!("音量浮层隐藏任务意外停止：{error}"))?
}

/// 修改当前媒体应用音量，并返回 Core Audio 实际采用的状态。
#[tauri::command]
pub fn control_current_application_volume(
    app: AppHandle,
    media: State<'_, SystemMediaManager>,
    volume: State<'_, ApplicationVolumeManager>,
    expected_session_key: u64,
    action: ApplicationVolumeAction,
) -> Result<ApplicationVolumeState, ApplicationVolumeError> {
    let identity = media
        .current_volume_identity(&app, expected_session_key)
        .map_err(ApplicationVolumeError::media_identity)?;
    volume.control(identity, action)
}
