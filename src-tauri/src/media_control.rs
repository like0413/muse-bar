use serde::{Deserialize, Serialize};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};

const TICKS_PER_MILLISECOND: i64 = 10_000;

/// 前端可以请求的媒体控制动作。
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ControlAction {
    TogglePlayPause,
    Previous,
    Next,
    Seek {
        #[serde(rename = "positionMs")]
        position_ms: i64,
    },
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

    if let ControlAction::Seek { position_ms } = action {
        let is_enabled = controls.IsPlaybackPositionEnabled().map_err(|error| {
            MediaControlError::new(
                action,
                MediaControlErrorCode::WindowsApi,
                format!("无法确认播放器是否支持进度跳转：{error}"),
            )
        })?;
        if !is_enabled {
            return Err(MediaControlError::new(
                action,
                MediaControlErrorCode::Unsupported,
                "当前播放器未声明支持进度跳转",
            ));
        }

        return execute_seek(session, action, position_ms);
    }

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
        ControlAction::Seek { .. } => unreachable!("seek 已在能力检查前单独处理"),
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
        ControlAction::Seek { .. } => unreachable!("seek 已在普通控制动作前单独处理"),
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

/// 将目标毫秒限制在播放器声明的有效范围内，再请求 Windows 改变播放位置。
fn execute_seek(
    session: &GlobalSystemMediaTransportControlsSession,
    action: ControlAction,
    position_ms: i64,
) -> Result<(), MediaControlError> {
    let timeline = session.GetTimelineProperties().map_err(|error| {
        MediaControlError::new(
            action,
            MediaControlErrorCode::WindowsApi,
            format!("无法读取当前播放器时间轴：{error}"),
        )
    })?;
    let start_ticks = timeline
        .StartTime()
        .map_err(|error| {
            MediaControlError::new(
                action,
                MediaControlErrorCode::WindowsApi,
                format!("无法读取时间轴开始位置：{error}"),
            )
        })?
        .Duration;
    let end_ticks = timeline
        .EndTime()
        .map_err(|error| {
            MediaControlError::new(
                action,
                MediaControlErrorCode::WindowsApi,
                format!("无法读取时间轴结束位置：{error}"),
            )
        })?
        .Duration;
    let min_seek_ticks = timeline
        .MinSeekTime()
        .map_err(|error| {
            MediaControlError::new(
                action,
                MediaControlErrorCode::WindowsApi,
                format!("无法读取最小进度跳转位置：{error}"),
            )
        })?
        .Duration;
    let max_seek_ticks = timeline
        .MaxSeekTime()
        .map_err(|error| {
            MediaControlError::new(
                action,
                MediaControlErrorCode::WindowsApi,
                format!("无法读取最大进度跳转位置：{error}"),
            )
        })?
        .Duration;

    // 某些播放器不填写 MaxSeekTime，此时用曲目终点兜底；有效范围始终不能越过起止时间。
    let lower_ticks = start_ticks.max(min_seek_ticks);
    let reported_upper_ticks = if max_seek_ticks > lower_ticks {
        max_seek_ticks
    } else {
        end_ticks
    };
    let upper_ticks = end_ticks.min(reported_upper_ticks);
    if upper_ticks <= lower_ticks {
        return Err(MediaControlError::new(
            action,
            MediaControlErrorCode::Unsupported,
            "当前媒体没有可跳转的有效时间轴",
        ));
    }

    let lower_ms = lower_ticks / TICKS_PER_MILLISECOND;
    let upper_ms = upper_ticks / TICKS_PER_MILLISECOND;
    let target_ms = position_ms.clamp(lower_ms, upper_ms);
    let target_ticks = target_ms
        .checked_mul(TICKS_PER_MILLISECOND)
        .ok_or_else(|| {
            MediaControlError::new(
                action,
                MediaControlErrorCode::WindowsApi,
                "目标播放位置无法转换为 Windows 时间单位",
            )
        })?;
    let accepted = session
        .TryChangePlaybackPositionAsync(target_ticks)
        .and_then(|operation| operation.get())
        .map_err(|error| {
            MediaControlError::new(
                action,
                MediaControlErrorCode::WindowsApi,
                format!("Windows 进度跳转调用失败：{error}"),
            )
        })?;

    if !accepted {
        return Err(MediaControlError::new(
            action,
            MediaControlErrorCode::Rejected,
            "播放器收到请求，但没有接受进度跳转",
        ));
    }

    Ok(())
}
