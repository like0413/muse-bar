use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus, MediaPropertiesChangedEventArgs,
    PlaybackInfoChangedEventArgs, SessionsChangedEventArgs,
};

const MEDIA_SESSIONS_CHANGED_EVENT: &str = "media-sessions-changed";
const CURRENT_MEDIA_METADATA_CHANGED_EVENT: &str = "current-media-metadata-changed";
const CURRENT_PLAYBACK_STATUS_CHANGED_EVENT: &str = "current-playback-status-changed";

/// 当前 Windows 系统会话可提供的基础文本元数据。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentMediaMetadata {
    source_app_id: String,
    title: String,
    artist: String,
}

/// Windows 当前媒体会话的播放状态。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CurrentPlaybackStatus {
    Closed,
    Opened,
    Changing,
    Stopped,
    Playing,
    Paused,
    Unknown,
}

/// 保存当前会话以及注销其元数据事件所需的 token。
#[derive(Default)]
struct CurrentSessionObservation {
    session: Option<GlobalSystemMediaTransportControlsSession>,
    media_properties_changed_token: Option<i64>,
    playback_info_changed_token: Option<i64>,
}

/// 保存整个应用进程唯一的 Windows 全局系统媒体管理器。
pub(crate) struct SystemMediaManager {
    manager: Option<GlobalSystemMediaTransportControlsSessionManager>,
    sessions_changed_token: Option<i64>,
    current_session_changed_token: Option<i64>,
    current_session_observation: Arc<Mutex<CurrentSessionObservation>>,
}

impl SystemMediaManager {
    /// 请求全局媒体管理器并订阅会话列表变化；失败时不阻止应用启动。
    pub(crate) fn initialize<R: Runtime>(app: &AppHandle<R>) -> Self {
        let current_session_observation =
            Arc::new(Mutex::new(CurrentSessionObservation::default()));
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .and_then(|operation| operation.get())
            .map_err(|error| {
                log::error!("无法初始化 Windows 全局系统媒体管理器：{error}");
            })
            .ok();
        let sessions_changed_token = manager
            .as_ref()
            .and_then(|manager| subscribe_to_sessions_changed(manager, app).ok());
        let current_session_changed_token = manager.as_ref().and_then(|manager| {
            subscribe_to_current_session_changed(
                manager,
                app,
                Arc::clone(&current_session_observation),
            )
            .ok()
        });

        if let Some(manager) = &manager {
            if let Err(error) = bind_current_session(manager, app, &current_session_observation) {
                log::error!("无法监听初始系统媒体会话：{error}");
            }
        }

        Self {
            manager,
            sessions_changed_token,
            current_session_changed_token,
            current_session_observation,
        }
    }

    /// 返回本次进程是否已经取得可供后续会话查询使用的管理器实例。
    pub(crate) fn is_initialized(&self) -> bool {
        self.manager.is_some()
    }

    /// 枚举当前系统媒体会话，并按 Windows 返回顺序提取 Source App ID。
    pub(crate) fn source_app_ids(&self) -> Result<Vec<String>, String> {
        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| "Windows 全局系统媒体管理器尚未初始化".to_owned())?;

        collect_source_app_ids(manager)
    }

    /// 读取 Windows 当前会话的标题和歌手；没有当前会话时返回空值。
    pub(crate) fn current_media_metadata(&self) -> Result<Option<CurrentMediaMetadata>, String> {
        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| "Windows 全局系统媒体管理器尚未初始化".to_owned())?;
        let Some(session) = manager.GetCurrentSession().ok() else {
            return Ok(None);
        };

        read_media_metadata(&session).map(Some)
    }

    /// 读取 Windows 当前会话的播放状态；没有当前会话时返回空值。
    pub(crate) fn current_playback_status(&self) -> Result<Option<CurrentPlaybackStatus>, String> {
        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| "Windows 全局系统媒体管理器尚未初始化".to_owned())?;
        let Some(session) = manager.GetCurrentSession().ok() else {
            return Ok(None);
        };

        read_playback_status(&session).map(Some)
    }
}

impl Drop for SystemMediaManager {
    /// 应用退出时注销 WinRT 事件，避免管理器继续持有已无用途的回调。
    fn drop(&mut self) {
        if let Some(manager) = &self.manager {
            if let Some(token) = self.sessions_changed_token {
                if let Err(error) = manager.RemoveSessionsChanged(token) {
                    log::warn!("无法注销系统媒体会话列表监听：{error}");
                }
            }
            if let Some(token) = self.current_session_changed_token {
                if let Err(error) = manager.RemoveCurrentSessionChanged(token) {
                    log::warn!("无法注销 Windows 当前媒体会话监听：{error}");
                }
            }
        }

        if let Ok(mut observation) = self.current_session_observation.lock() {
            clear_current_session_observation(&mut observation);
        }
    }
}

/// 注册会话列表变化事件，并将最新 Source App ID 广播给所有前端窗口。
fn subscribe_to_sessions_changed<R: Runtime>(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    app: &AppHandle<R>,
) -> Result<i64, String> {
    let app = app.clone();
    // `TypedEventHandler::new` 无法从闭包体反推出两个 WinRT 泛型，
    // 因此在变量上明确写出事件发送者和事件参数类型。
    let handler: TypedEventHandler<
        GlobalSystemMediaTransportControlsSessionManager,
        SessionsChangedEventArgs,
    > = TypedEventHandler::new(
        move |sender: windows::core::Ref<'_, GlobalSystemMediaTransportControlsSessionManager>,
              _: windows::core::Ref<'_, SessionsChangedEventArgs>| {
            let Some(manager) = sender.as_ref() else {
                return Ok(());
            };

            match collect_source_app_ids(manager) {
                Ok(source_app_ids) => {
                    if let Err(error) = app.emit(MEDIA_SESSIONS_CHANGED_EVENT, source_app_ids) {
                        log::warn!("无法广播系统媒体会话列表变化：{error}");
                    }
                }
                Err(error) => log::warn!("系统媒体会话列表变化后重新枚举失败：{error}"),
            }

            Ok(())
        },
    );

    manager.SessionsChanged(&handler).map_err(|error| {
        let message = format!("无法订阅系统媒体会话列表变化：{error}");
        log::error!("{message}");
        message
    })
}

/// 监听 Windows 当前媒体会话变化，并将元数据监听切换到新的会话。
fn subscribe_to_current_session_changed<R: Runtime>(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    app: &AppHandle<R>,
    observation: Arc<Mutex<CurrentSessionObservation>>,
) -> Result<i64, String> {
    let app = app.clone();
    let handler: TypedEventHandler<
        GlobalSystemMediaTransportControlsSessionManager,
        CurrentSessionChangedEventArgs,
    > = TypedEventHandler::new(
        move |sender: windows::core::Ref<'_, GlobalSystemMediaTransportControlsSessionManager>,
              _: windows::core::Ref<'_, CurrentSessionChangedEventArgs>| {
            let Some(manager) = sender.as_ref() else {
                return Ok(());
            };

            if let Err(error) = bind_current_session(manager, &app, &observation) {
                log::warn!("切换 Windows 当前媒体会话监听失败：{error}");
            }

            Ok(())
        },
    );

    manager.CurrentSessionChanged(&handler).map_err(|error| {
        let message = format!("无法订阅 Windows 当前媒体会话变化：{error}");
        log::error!("{message}");
        message
    })
}

/// 注销旧会话并绑定 Windows 当前会话的元数据变化事件。
fn bind_current_session<R: Runtime>(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    app: &AppHandle<R>,
    observation: &Arc<Mutex<CurrentSessionObservation>>,
) -> Result<(), String> {
    let current_session = manager.GetCurrentSession().ok();
    let mut observation = observation
        .lock()
        .map_err(|_| "当前媒体会话监听状态锁已损坏".to_owned())?;
    clear_current_session_observation(&mut observation);

    let Some(session) = current_session else {
        app.emit(
            CURRENT_MEDIA_METADATA_CHANGED_EVENT,
            Option::<CurrentMediaMetadata>::None,
        )
        .map_err(|error| format!("无法广播当前媒体会话已清空：{error}"))?;
        app.emit(
            CURRENT_PLAYBACK_STATUS_CHANGED_EVENT,
            Option::<CurrentPlaybackStatus>::None,
        )
        .map_err(|error| format!("无法广播当前播放状态已清空：{error}"))?;
        return Ok(());
    };

    let event_app = app.clone();
    let handler: TypedEventHandler<
        GlobalSystemMediaTransportControlsSession,
        MediaPropertiesChangedEventArgs,
    > = TypedEventHandler::new(
        move |sender: windows::core::Ref<'_, GlobalSystemMediaTransportControlsSession>,
              _: windows::core::Ref<'_, MediaPropertiesChangedEventArgs>| {
            let Some(session) = sender.as_ref() else {
                return Ok(());
            };

            emit_media_metadata(session, &event_app);
            Ok(())
        },
    );
    let token = session
        .MediaPropertiesChanged(&handler)
        .map_err(|error| format!("无法订阅当前媒体会话元数据变化：{error}"))?;

    let playback_app = app.clone();
    let playback_handler: TypedEventHandler<
        GlobalSystemMediaTransportControlsSession,
        PlaybackInfoChangedEventArgs,
    > = TypedEventHandler::new(
        move |sender: windows::core::Ref<'_, GlobalSystemMediaTransportControlsSession>,
              _: windows::core::Ref<'_, PlaybackInfoChangedEventArgs>| {
            let Some(session) = sender.as_ref() else {
                return Ok(());
            };

            emit_playback_status(session, &playback_app);
            Ok(())
        },
    );
    let playback_token = match session.PlaybackInfoChanged(&playback_handler) {
        Ok(token) => token,
        Err(error) => {
            let _ = session.RemoveMediaPropertiesChanged(token);
            return Err(format!("无法订阅当前媒体会话播放状态变化：{error}"));
        }
    };

    observation.session = Some(session.clone());
    observation.media_properties_changed_token = Some(token);
    observation.playback_info_changed_token = Some(playback_token);
    drop(observation);

    emit_media_metadata(&session, app);
    emit_playback_status(&session, app);
    Ok(())
}

/// 注销并清空当前会话观察对象；注销失败只记录日志，不影响后续重新绑定。
fn clear_current_session_observation(observation: &mut CurrentSessionObservation) {
    if let (Some(session), Some(token)) = (
        &observation.session,
        observation.media_properties_changed_token,
    ) {
        if let Err(error) = session.RemoveMediaPropertiesChanged(token) {
            log::warn!("无法注销当前媒体会话元数据监听：{error}");
        }
    }
    if let (Some(session), Some(token)) = (
        &observation.session,
        observation.playback_info_changed_token,
    ) {
        if let Err(error) = session.RemovePlaybackInfoChanged(token) {
            log::warn!("无法注销当前媒体会话播放状态监听：{error}");
        }
    }

    observation.session = None;
    observation.media_properties_changed_token = None;
    observation.playback_info_changed_token = None;
}

/// 读取会话文本元数据并广播；事件回调中的读取失败只记录日志。
fn emit_media_metadata<R: Runtime>(
    session: &GlobalSystemMediaTransportControlsSession,
    app: &AppHandle<R>,
) {
    match read_media_metadata(session) {
        Ok(metadata) => {
            if let Err(error) = app.emit(CURRENT_MEDIA_METADATA_CHANGED_EVENT, Some(metadata)) {
                log::warn!("无法广播当前媒体会话元数据：{error}");
            }
        }
        Err(error) => log::warn!("无法在媒体变化后读取标题和歌手：{error}"),
    }
}

/// 读取当前会话播放状态并广播；事件回调中的读取失败只记录日志。
fn emit_playback_status<R: Runtime>(
    session: &GlobalSystemMediaTransportControlsSession,
    app: &AppHandle<R>,
) {
    match read_playback_status(session) {
        Ok(status) => {
            if let Err(error) = app.emit(CURRENT_PLAYBACK_STATUS_CHANGED_EVENT, Some(status)) {
                log::warn!("无法广播当前媒体会话播放状态：{error}");
            }
        }
        Err(error) => log::warn!("无法在播放信息变化后读取状态：{error}"),
    }
}

/// 调用 WinRT 异步媒体属性接口并提取 Source App ID、标题和歌手。
fn read_media_metadata(
    session: &GlobalSystemMediaTransportControlsSession,
) -> Result<CurrentMediaMetadata, String> {
    let source_app_id = session
        .SourceAppUserModelId()
        .map_err(|error| format!("无法读取当前会话的 Source App ID：{error}"))?;
    let properties = session
        .TryGetMediaPropertiesAsync()
        .and_then(|operation| operation.get())
        .map_err(|error| format!("无法读取当前会话的媒体属性：{error}"))?;
    let title = properties
        .Title()
        .map_err(|error| format!("无法读取当前会话标题：{error}"))?;
    let artist = properties
        .Artist()
        .map_err(|error| format!("无法读取当前会话歌手：{error}"))?;

    Ok(CurrentMediaMetadata {
        source_app_id: source_app_id.to_string(),
        title: title.to_string(),
        artist: artist.to_string(),
    })
}

/// 读取 WinRT 播放信息，并转换为可稳定序列化的应用状态。
fn read_playback_status(
    session: &GlobalSystemMediaTransportControlsSession,
) -> Result<CurrentPlaybackStatus, String> {
    let playback_info = session
        .GetPlaybackInfo()
        .map_err(|error| format!("无法读取当前会话播放信息：{error}"))?;
    let status = playback_info
        .PlaybackStatus()
        .map_err(|error| format!("无法读取当前会话播放状态：{error}"))?;

    let status = if status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Closed {
        CurrentPlaybackStatus::Closed
    } else if status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Opened {
        CurrentPlaybackStatus::Opened
    } else if status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Changing {
        CurrentPlaybackStatus::Changing
    } else if status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped {
        CurrentPlaybackStatus::Stopped
    } else if status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing {
        CurrentPlaybackStatus::Playing
    } else if status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused {
        CurrentPlaybackStatus::Paused
    } else {
        CurrentPlaybackStatus::Unknown
    };

    Ok(status)
}

/// 从指定管理器读取全部会话，并提取 Source App ID。
fn collect_source_app_ids(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
) -> Result<Vec<String>, String> {
    let sessions = manager
        .GetSessions()
        .map_err(|error| format!("无法枚举系统媒体会话：{error}"))?;
    let session_count = sessions
        .Size()
        .map_err(|error| format!("无法读取系统媒体会话数量：{error}"))?;
    let mut source_app_ids = Vec::with_capacity(session_count as usize);

    for index in 0..session_count {
        let session = sessions
            .GetAt(index)
            .map_err(|error| format!("无法读取第 {} 个系统媒体会话：{error}", index + 1))?;
        let source_app_id = session
            .SourceAppUserModelId()
            .map_err(|error| format!("无法读取第 {} 个会话的 Source App ID：{error}", index + 1))?;
        source_app_ids.push(source_app_id.to_string());
    }

    Ok(source_app_ids)
}
