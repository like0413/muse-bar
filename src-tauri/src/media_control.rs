use serde::{Deserialize, Serialize};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};

/// 前端可以请求的媒体控制动作。
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ControlAction {
    TogglePlayPause,
    Previous,
    Next,
}

/// 媒体控制失败的稳定错误类别。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MediaControlErrorCode {
    NoSession,
    Unsupported,
    Rejected,
    WindowsApi,
}

/// 返回给前端的结构化媒体控制错误。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaControlError {
    action: ControlAction,
    code: MediaControlErrorCode,
    message: String,
}

impl MediaControlError {
    /// 使用动作、错误类别和可读说明创建控制错误。
    fn new(action: ControlAction, code: MediaControlErrorCode, message: impl Into<String>) -> Self {
        Self {
            action,
            code,
            message: message.into(),
        }
    }

    /// 将应用内部状态读取失败转换为统一的 Windows API 控制错误。
    pub(crate) fn windows_api(action: ControlAction, message: impl Into<String>) -> Self {
        Self::new(action, MediaControlErrorCode::WindowsApi, message)
    }
}

/// 检查播放器能力后，对当前选中会话执行一个媒体控制动作。
pub(crate) fn execute_control_action(
    session: Option<&GlobalSystemMediaTransportControlsSession>,
    action: ControlAction,
) -> Result<(), MediaControlError> {
    let session = session.ok_or_else(|| {
        MediaControlError::new(
            action,
            MediaControlErrorCode::NoSession,
            "当前没有可控制的媒体会话",
        )
    })?;
    let playback_info = session.GetPlaybackInfo().map_err(|error| {
        MediaControlError::new(
            action,
            MediaControlErrorCode::WindowsApi,
            format!("无法读取当前播放器状态：{error}"),
        )
    })?;
    let controls = playback_info.Controls().map_err(|error| {
        MediaControlError::new(
            action,
            MediaControlErrorCode::WindowsApi,
            format!("无法读取当前播放器控制能力：{error}"),
        )
    })?;

    let is_enabled = match action {
        ControlAction::TogglePlayPause => {
            let status = playback_info.PlaybackStatus().map_err(|error| {
                MediaControlError::new(
                    action,
                    MediaControlErrorCode::WindowsApi,
                    format!("无法读取播放/暂停状态：{error}"),
                )
            })?;
            if status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing {
                controls.IsPauseEnabled()
            } else {
                controls.IsPlayEnabled()
            }
        }
        ControlAction::Previous => controls.IsPreviousEnabled(),
        ControlAction::Next => controls.IsNextEnabled(),
    }
    .map_err(|error| {
        MediaControlError::new(
            action,
            MediaControlErrorCode::WindowsApi,
            format!("无法确认播放器是否支持当前操作：{error}"),
        )
    })?;

    if !is_enabled {
        return Err(MediaControlError::new(
            action,
            MediaControlErrorCode::Unsupported,
            "当前播放器未声明支持此操作",
        ));
    }

    let accepted = match action {
        ControlAction::TogglePlayPause => session.TryTogglePlayPauseAsync(),
        ControlAction::Previous => session.TrySkipPreviousAsync(),
        ControlAction::Next => session.TrySkipNextAsync(),
    }
    .and_then(|operation| operation.get())
    .map_err(|error| {
        MediaControlError::new(
            action,
            MediaControlErrorCode::WindowsApi,
            format!("Windows 媒体控制调用失败：{error}"),
        )
    })?;

    if !accepted {
        return Err(MediaControlError::new(
            action,
            MediaControlErrorCode::Rejected,
            "播放器收到请求，但没有接受此操作",
        ));
    }

    Ok(())
}
