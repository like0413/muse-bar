use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackInfo,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus, MediaPropertiesChangedEventArgs,
    PlaybackInfoChangedEventArgs, SessionsChangedEventArgs, TimelinePropertiesChangedEventArgs,
};
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

const TICKS_PER_MILLISECOND: i64 = 10_000;
const WINDOWS_TO_UNIX_EPOCH_TICKS: i64 = 116_444_736_000_000_000;
const MEDIA_SESSIONS_CHANGED_EVENT: &str = "media-sessions-changed";
const CURRENT_MEDIA_METADATA_CHANGED_EVENT: &str = "current-media-metadata-changed";
const CURRENT_PLAYBACK_STATUS_CHANGED_EVENT: &str = "current-playback-status-changed";
const CURRENT_PLAYBACK_CAPABILITIES_CHANGED_EVENT: &str = "current-playback-capabilities-changed";
const CURRENT_TIMELINE_CHANGED_EVENT: &str = "current-timeline-changed";
const MAX_ARTWORK_BYTES: u64 = 4 * 1024 * 1024;

/// 当前 Windows 系统会话提供的媒体元数据；封面与文字始终属于同一份快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentMediaMetadata {
    source_app_id: String,
    title: String,
    artist: String,
    artwork_data_url: Option<String>,
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

/// Windows 当前媒体会话声明支持的控制能力。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentPlaybackCapabilities {
    can_play: bool,
    can_pause: bool,
    can_previous: bool,
    can_next: bool,
    can_seek: bool,
}

/// Windows 当前媒体会话上报的有效时间轴快照。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentTimeline {
    start_ms: i64,
    end_ms: i64,
    position_ms: i64,
    min_seek_ms: i64,
    max_seek_ms: i64,
    last_updated_at_unix_ms: i64,
    playback_rate: Option<f64>,
}

/// 保存当前会话以及注销其元数据事件所需的 token。
#[derive(Default)]
struct CurrentSessionObservation {
    session: Option<GlobalSystemMediaTransportControlsSession>,
    media_properties_changed_token: Option<i64>,
    playback_info_changed_token: Option<i64>,
    timeline_properties_changed_token: Option<i64>,
}

/// 交给固定 WinRT apartment 的元数据读取任务。
struct MediaMetadataRequest {
    session: GlobalSystemMediaTransportControlsSession,
    revision: u64,
}

/// 保存异步媒体属性任务队列、最近快照和版本号。
struct MediaMetadataLoader {
    sender: Sender<MediaMetadataRequest>,
    cached: Arc<Mutex<Option<CurrentMediaMetadata>>>,
    revision: Arc<AtomicU64>,
}

impl MediaMetadataLoader {
    /// 创建唯一的 MTA 工作线程，让不可跨线程的 WinRT 流始终留在同一 apartment。
    fn start<R: Runtime>(app: &AppHandle<R>) -> Self {
        let (sender, receiver) = mpsc::channel();
        let cached = Arc::new(Mutex::new(None));
        let revision = Arc::new(AtomicU64::new(0));
        let worker_app = app.clone();
        let worker_cached = Arc::clone(&cached);
        let worker_revision = Arc::clone(&revision);

        if let Err(error) = thread::Builder::new()
            .name("muse-bar-media-metadata".to_owned())
            .spawn(move || {
                run_media_metadata_worker(receiver, worker_cached, worker_revision, worker_app);
            })
        {
            log::error!("无法启动媒体元数据异步线程：{error}");
        }

        Self {
            sender,
            cached,
            revision,
        }
    }

    /// 提交一次完整元数据读取；这里只入队，不等待任何 WinRT 操作。
    fn request(&self, session: &GlobalSystemMediaTransportControlsSession) {
        let revision = self.revision.fetch_add(1, Ordering::AcqRel) + 1;
        let request = MediaMetadataRequest {
            session: session.clone(),
            revision,
        };

        if let Err(error) = self.sender.send(request) {
            log::warn!("无法提交媒体元数据读取任务：{error}");
        }
    }

    /// 当前会话消失时使在途任务失效，并清空完整元数据缓存。
    fn clear(&self) -> Result<(), String> {
        self.revision.fetch_add(1, Ordering::AcqRel);
        let mut cached = self
            .cached
            .lock()
            .map_err(|_| "当前媒体元数据缓存锁已损坏".to_owned())?;
        *cached = None;
        Ok(())
    }

    /// 返回后台线程最近完成的完整元数据快照。
    fn cached(&self) -> Result<Option<CurrentMediaMetadata>, String> {
        self.cached
            .lock()
            .map(|cached| cached.clone())
            .map_err(|_| "当前媒体元数据缓存锁已损坏".to_owned())
    }
}

/// 在固定 MTA apartment 内异步读取完整 MediaProperties，并丢弃过期切歌结果。
fn run_media_metadata_worker<R: Runtime>(
    receiver: Receiver<MediaMetadataRequest>,
    cached: Arc<Mutex<Option<CurrentMediaMetadata>>>,
    revision: Arc<AtomicU64>,
    app: AppHandle<R>,
) {
    // 原始线程没有 COM apartment；WinRT 媒体属性与缩略图流必须在初始化后使用。
    if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
        log::error!("无法初始化媒体元数据线程的 WinRT 环境：{error}");
        return;
    }

    while let Ok(mut request) = receiver.recv() {
        // 快速连续切歌时跳过尚未开始的旧任务，直接处理最新媒体属性。
        while let Ok(newer_request) = receiver.try_recv() {
            request = newer_request;
        }

        let result = read_media_metadata(&request.session);
        if revision.load(Ordering::Acquire) != request.revision {
            continue;
        }

        match result {
            Ok(metadata) => {
                let cache_updated = cached
                    .lock()
                    .map(|mut cached| *cached = Some(metadata.clone()))
                    .is_ok();
                if !cache_updated {
                    log::warn!("媒体元数据读取完成，但缓存锁已损坏");
                    continue;
                }

                if let Err(error) = app.emit(CURRENT_MEDIA_METADATA_CHANGED_EVENT, Some(metadata)) {
                    log::warn!("无法广播当前媒体会话元数据：{error}");
                }
            }
            Err(error) => {
                if let Ok(mut cached) = cached.lock() {
                    *cached = None;
                }
                if let Err(emit_error) = app.emit(
                    CURRENT_MEDIA_METADATA_CHANGED_EVENT,
                    Option::<CurrentMediaMetadata>::None,
                ) {
                    log::warn!("无法广播媒体元数据读取失败：{emit_error}");
                }
                log::warn!("无法异步读取当前媒体元数据：{error}");
            }
        }
    }

    // SAFETY: 本线程成功初始化过一次 WinRT apartment，并且即将退出。
    unsafe { RoUninitialize() };
}

/// 保存整个应用进程唯一的 Windows 全局系统媒体管理器。
pub(crate) struct SystemMediaManager {
    manager: Option<GlobalSystemMediaTransportControlsSessionManager>,
    sessions_changed_token: Option<i64>,
    current_session_changed_token: Option<i64>,
    current_session_observation: Arc<Mutex<CurrentSessionObservation>>,
    media_metadata_loader: Arc<MediaMetadataLoader>,
}

impl SystemMediaManager {
    /// 请求全局媒体管理器并订阅会话列表变化；失败时不阻止应用启动。
    pub(crate) fn initialize<R: Runtime>(app: &AppHandle<R>) -> Self {
        let current_session_observation =
            Arc::new(Mutex::new(CurrentSessionObservation::default()));
        let media_metadata_loader = Arc::new(MediaMetadataLoader::start(app));
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
                Arc::clone(&media_metadata_loader),
            )
            .ok()
        });

        if let Some(manager) = &manager {
            if let Err(error) = bind_current_session(
                manager,
                app,
                &current_session_observation,
                &media_metadata_loader,
            ) {
                log::error!("无法监听初始系统媒体会话：{error}");
            }
        }

        Self {
            manager,
            sessions_changed_token,
            current_session_changed_token,
            current_session_observation,
            media_metadata_loader,
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

    /// 从内存缓存读取标题、歌手和封面，不在 Tauri 命令线程中等待 WinRT。
    pub(crate) fn current_media_metadata(&self) -> Result<Option<CurrentMediaMetadata>, String> {
        self.media_metadata_loader.cached()
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

    /// 读取 Windows 当前会话声明的控制能力；没有当前会话时返回空值。
    pub(crate) fn current_playback_capabilities(
        &self,
    ) -> Result<Option<CurrentPlaybackCapabilities>, String> {
        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| "Windows 全局系统媒体管理器尚未初始化".to_owned())?;
        let Some(session) = manager.GetCurrentSession().ok() else {
            return Ok(None);
        };

        read_playback_capabilities(&session).map(Some)
    }

    /// 读取 Windows 当前会话的有效时间轴；没有会话或时间轴无效时返回空值。
    pub(crate) fn current_timeline(&self) -> Result<Option<CurrentTimeline>, String> {
        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| "Windows 全局系统媒体管理器尚未初始化".to_owned())?;
        let Some(session) = manager.GetCurrentSession().ok() else {
            return Ok(None);
        };

        read_timeline(&session)
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
    media_metadata_loader: Arc<MediaMetadataLoader>,
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

            if let Err(error) =
                bind_current_session(manager, &app, &observation, &media_metadata_loader)
            {
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
    media_metadata_loader: &Arc<MediaMetadataLoader>,
) -> Result<(), String> {
    let current_session = manager.GetCurrentSession().ok();
    let mut observation = observation
        .lock()
        .map_err(|_| "当前媒体会话监听状态锁已损坏".to_owned())?;
    clear_current_session_observation(&mut observation);

    let Some(session) = current_session else {
        media_metadata_loader.clear()?;
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
        app.emit(
            CURRENT_PLAYBACK_CAPABILITIES_CHANGED_EVENT,
            Option::<CurrentPlaybackCapabilities>::None,
        )
        .map_err(|error| format!("无法广播当前控制能力已清空：{error}"))?;
        app.emit(
            CURRENT_TIMELINE_CHANGED_EVENT,
            Option::<CurrentTimeline>::None,
        )
        .map_err(|error| format!("无法广播当前时间轴已清空：{error}"))?;
        return Ok(());
    };

    let event_metadata_loader = Arc::clone(media_metadata_loader);
    let handler: TypedEventHandler<
        GlobalSystemMediaTransportControlsSession,
        MediaPropertiesChangedEventArgs,
    > = TypedEventHandler::new(
        move |sender: windows::core::Ref<'_, GlobalSystemMediaTransportControlsSession>,
              _: windows::core::Ref<'_, MediaPropertiesChangedEventArgs>| {
            let Some(session) = sender.as_ref() else {
                return Ok(());
            };

            event_metadata_loader.request(session);
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

            emit_playback_info(session, &playback_app);
            emit_timeline(session, &playback_app);
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

    let timeline_app = app.clone();
    let timeline_handler: TypedEventHandler<
        GlobalSystemMediaTransportControlsSession,
        TimelinePropertiesChangedEventArgs,
    > = TypedEventHandler::new(
        move |sender: windows::core::Ref<'_, GlobalSystemMediaTransportControlsSession>,
              _: windows::core::Ref<'_, TimelinePropertiesChangedEventArgs>| {
            let Some(session) = sender.as_ref() else {
                return Ok(());
            };

            emit_timeline(session, &timeline_app);
            Ok(())
        },
    );
    let timeline_token = match session.TimelinePropertiesChanged(&timeline_handler) {
        Ok(token) => token,
        Err(error) => {
            let _ = session.RemoveMediaPropertiesChanged(token);
            let _ = session.RemovePlaybackInfoChanged(playback_token);
            return Err(format!("无法订阅当前媒体会话时间轴变化：{error}"));
        }
    };

    observation.session = Some(session.clone());
    observation.media_properties_changed_token = Some(token);
    observation.playback_info_changed_token = Some(playback_token);
    observation.timeline_properties_changed_token = Some(timeline_token);
    drop(observation);

    media_metadata_loader.request(&session);
    emit_playback_info(&session, app);
    emit_timeline(&session, app);
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
        observation.timeline_properties_changed_token,
    ) {
        if let Err(error) = session.RemoveTimelinePropertiesChanged(token) {
            log::warn!("无法注销当前媒体会话时间轴监听：{error}");
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
    observation.timeline_properties_changed_token = None;
}

/// 一次读取当前播放信息，并分别广播状态和控制能力。
fn emit_playback_info<R: Runtime>(
    session: &GlobalSystemMediaTransportControlsSession,
    app: &AppHandle<R>,
) {
    let playback_info = match session.GetPlaybackInfo() {
        Ok(playback_info) => playback_info,
        Err(error) => {
            log::warn!("无法在播放信息变化后读取当前会话：{error}");
            return;
        }
    };

    match playback_status_from_info(&playback_info) {
        Ok(status) => {
            if let Err(error) = app.emit(CURRENT_PLAYBACK_STATUS_CHANGED_EVENT, Some(status)) {
                log::warn!("无法广播当前媒体会话播放状态：{error}");
            }
        }
        Err(error) => log::warn!("无法在播放信息变化后读取状态：{error}"),
    }

    match playback_capabilities_from_info(&playback_info) {
        Ok(capabilities) => {
            if let Err(error) = app.emit(
                CURRENT_PLAYBACK_CAPABILITIES_CHANGED_EVENT,
                Some(capabilities),
            ) {
                log::warn!("无法广播当前媒体会话控制能力：{error}");
            }
        }
        Err(error) => log::warn!("无法在播放信息变化后读取控制能力：{error}"),
    }
}

/// 读取当前会话时间轴并广播；无效时间轴使用空值表示。
fn emit_timeline<R: Runtime>(
    session: &GlobalSystemMediaTransportControlsSession,
    app: &AppHandle<R>,
) {
    match read_timeline(session) {
        Ok(timeline) => {
            if let Err(error) = app.emit(CURRENT_TIMELINE_CHANGED_EVENT, timeline) {
                log::warn!("无法广播当前媒体会话时间轴：{error}");
            }
        }
        Err(error) => log::warn!("无法在时间轴变化后读取快照：{error}"),
    }
}

/// 一次取得 WinRT 媒体属性，并从同一份属性中提取标题、歌手和封面。
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
    let artwork_data_url = match properties.Thumbnail() {
        Ok(thumbnail) => match read_artwork_data_url(&thumbnail) {
            Ok(data_url) => data_url,
            Err(error) => {
                // 个别播放器会发布暂时不可读的缩略图。封面失败不应丢弃同批标题和歌手。
                log::warn!("无法读取当前会话封面，本次使用空封面：{error}");
                None
            }
        },
        Err(_) => None,
    };

    Ok(CurrentMediaMetadata {
        source_app_id: source_app_id.to_string(),
        title: title.to_string(),
        artist: artist.to_string(),
        artwork_data_url,
    })
}

/// 读取 SMTC 缩略图流并编码为 WebView 可直接显示的 data URL。
fn read_artwork_data_url(
    thumbnail: &IRandomAccessStreamReference,
) -> Result<Option<String>, String> {
    let stream = thumbnail
        .OpenReadAsync()
        .and_then(|operation| operation.get())
        .map_err(|error| format!("无法打开媒体封面流：{error}"))?;
    let size = stream
        .Size()
        .map_err(|error| format!("无法读取媒体封面大小：{error}"))?;

    if size == 0 {
        return Ok(None);
    }
    if size > MAX_ARTWORK_BYTES {
        return Err(format!(
            "媒体封面大小为 {size} 字节，超过 {MAX_ARTWORK_BYTES} 字节限制"
        ));
    }

    let byte_count = u32::try_from(size).map_err(|_| "媒体封面大小无法转换为 u32".to_owned())?;
    let input_stream = stream
        .GetInputStreamAt(0)
        .map_err(|error| format!("无法定位媒体封面流：{error}"))?;
    let reader = DataReader::CreateDataReader(&input_stream)
        .map_err(|error| format!("无法创建媒体封面读取器：{error}"))?;
    let loaded_count = reader
        .LoadAsync(byte_count)
        .and_then(|operation| operation.get())
        .map_err(|error| format!("无法载入媒体封面字节：{error}"))?;
    let mut bytes = vec![0; loaded_count as usize];
    reader
        .ReadBytes(&mut bytes)
        .map_err(|error| format!("无法复制媒体封面字节：{error}"))?;

    if bytes.is_empty() {
        return Ok(None);
    }

    let reported_content_type = stream
        .ContentType()
        .map(|content_type| content_type.to_string())
        .unwrap_or_default();
    let content_type = detect_artwork_content_type(&bytes, &reported_content_type);
    let encoded = BASE64_STANDARD.encode(bytes);

    Ok(Some(format!("data:{content_type};base64,{encoded}")))
}

/// 根据图片文件头确定 WebView 使用的 MIME；无法识别时才采用播放器上报的首个类型。
fn detect_artwork_content_type(bytes: &[u8], reported_content_type: &str) -> String {
    let detected = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else {
        None
    };

    if let Some(content_type) = detected {
        return content_type.to_owned();
    }

    // 有些播放器会返回逗号分隔的扩展名列表。data URL 只接受单一 MIME，
    // 因此最多采用第一个不带参数的 image/* 类型。
    let reported = reported_content_type
        .split([',', ';'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if reported.starts_with("image/") {
        reported
    } else {
        "image/jpeg".to_owned()
    }
}

/// 读取 WinRT 播放信息，并转换为可稳定序列化的应用状态。
fn read_playback_status(
    session: &GlobalSystemMediaTransportControlsSession,
) -> Result<CurrentPlaybackStatus, String> {
    let playback_info = session
        .GetPlaybackInfo()
        .map_err(|error| format!("无法读取当前会话播放信息：{error}"))?;

    playback_status_from_info(&playback_info)
}

/// 从已取得的播放信息中转换应用播放状态。
fn playback_status_from_info(
    playback_info: &GlobalSystemMediaTransportControlsSessionPlaybackInfo,
) -> Result<CurrentPlaybackStatus, String> {
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

/// 读取当前会话播放信息中的全部控制能力。
fn read_playback_capabilities(
    session: &GlobalSystemMediaTransportControlsSession,
) -> Result<CurrentPlaybackCapabilities, String> {
    let playback_info = session
        .GetPlaybackInfo()
        .map_err(|error| format!("无法读取当前会话播放信息：{error}"))?;

    playback_capabilities_from_info(&playback_info)
}

/// 从已取得的播放信息中读取播放、暂停、切歌和 seek 能力。
fn playback_capabilities_from_info(
    playback_info: &GlobalSystemMediaTransportControlsSessionPlaybackInfo,
) -> Result<CurrentPlaybackCapabilities, String> {
    let controls = playback_info
        .Controls()
        .map_err(|error| format!("无法读取当前会话控制能力：{error}"))?;

    Ok(CurrentPlaybackCapabilities {
        can_play: controls
            .IsPlayEnabled()
            .map_err(|error| format!("无法读取播放能力：{error}"))?,
        can_pause: controls
            .IsPauseEnabled()
            .map_err(|error| format!("无法读取暂停能力：{error}"))?,
        can_previous: controls
            .IsPreviousEnabled()
            .map_err(|error| format!("无法读取上一曲能力：{error}"))?,
        can_next: controls
            .IsNextEnabled()
            .map_err(|error| format!("无法读取下一曲能力：{error}"))?,
        can_seek: controls
            .IsPlaybackPositionEnabled()
            .map_err(|error| format!("无法读取 seek 能力：{error}"))?,
    })
}

/// 读取并校验当前会话时间轴，将 Windows ticks 转换为毫秒。
fn read_timeline(
    session: &GlobalSystemMediaTransportControlsSession,
) -> Result<Option<CurrentTimeline>, String> {
    let timeline = session
        .GetTimelineProperties()
        .map_err(|error| format!("无法读取当前会话时间轴：{error}"))?;
    let start_ticks = timeline
        .StartTime()
        .map_err(|error| format!("无法读取时间轴开始时间：{error}"))?
        .Duration;
    let end_ticks = timeline
        .EndTime()
        .map_err(|error| format!("无法读取时间轴结束时间：{error}"))?
        .Duration;

    // 直播、尚未开始播放或未实现时间轴的播放器通常返回相同的起止值。
    // 这类数据无法计算有效进度，因此明确返回空值。
    if end_ticks <= start_ticks {
        return Ok(None);
    }

    let position_ticks = timeline
        .Position()
        .map_err(|error| format!("无法读取时间轴当前位置：{error}"))?
        .Duration;
    let min_seek_ticks = timeline
        .MinSeekTime()
        .map_err(|error| format!("无法读取时间轴最小 seek 位置：{error}"))?
        .Duration;
    let max_seek_ticks = timeline
        .MaxSeekTime()
        .map_err(|error| format!("无法读取时间轴最大 seek 位置：{error}"))?
        .Duration;
    let last_updated_ticks = timeline
        .LastUpdatedTime()
        .map_err(|error| format!("无法读取时间轴最后更新时间：{error}"))?
        .UniversalTime;
    let last_updated_at_unix_ms = last_updated_ticks
        .checked_sub(WINDOWS_TO_UNIX_EPOCH_TICKS)
        .ok_or_else(|| "时间轴最后更新时间早于 Windows 时间原点".to_owned())?
        / TICKS_PER_MILLISECOND;
    let playback_rate = session
        .GetPlaybackInfo()
        .ok()
        .and_then(|info| info.PlaybackRate().ok())
        .and_then(|rate| rate.Value().ok())
        .filter(|rate| rate.is_finite());

    Ok(Some(CurrentTimeline {
        start_ms: ticks_to_milliseconds(start_ticks),
        end_ms: ticks_to_milliseconds(end_ticks),
        position_ms: ticks_to_milliseconds(position_ticks),
        min_seek_ms: ticks_to_milliseconds(min_seek_ticks),
        max_seek_ms: ticks_to_milliseconds(max_seek_ticks),
        last_updated_at_unix_ms,
        playback_rate,
    }))
}

/// 将 Windows TimeSpan 使用的 100ns ticks 转换为毫秒。
fn ticks_to_milliseconds(ticks: i64) -> i64 {
    ticks / TICKS_PER_MILLISECOND
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
