use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use crate::{
    media_activity::{
        session_key, MediaActivityTracker, MediaSessionActivity, SelectedMediaSession,
    },
    media_control::{execute_control_action, ControlAction, MediaControlError},
    platform::windows::{DwmGetColorizationColor, BOOL},
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
const MEDIA_SESSION_IDENTITIES_CHANGED_EVENT: &str = "media-session-identities-changed";
const CURRENT_MEDIA_METADATA_CHANGED_EVENT: &str = "current-media-metadata-changed";
const CURRENT_PLAYBACK_STATUS_CHANGED_EVENT: &str = "current-playback-status-changed";
const CURRENT_PLAYBACK_CAPABILITIES_CHANGED_EVENT: &str = "current-playback-capabilities-changed";
const CURRENT_TIMELINE_CHANGED_EVENT: &str = "current-timeline-changed";
const CURRENT_MEDIA_SNAPSHOT_CHANGED_EVENT: &str = "current-media-snapshot-changed";
const MAX_ARTWORK_BYTES: u64 = 4 * 1024 * 1024;
const DEFAULT_WINDOWS_ACCENT_COLOR: &str = "#0078D4";

/// 当前 Windows 系统会话提供的媒体元数据；封面与文字始终属于同一份快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentMediaMetadata {
    source_app_id: String,
    title: String,
    artist: String,
    artwork_data_url: Option<String>,
    accent_color: String,
}

/// Muse Bar 用于会话选择和诊断的播放器类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum MediaPlayerKind {
    #[serde(rename = "qqMusic")]
    QqMusic,
    #[serde(rename = "neteaseCloudMusic")]
    NeteaseCloudMusic,
    #[serde(rename = "kugouMusic")]
    KugouMusic,
    #[serde(rename = "qishuiMusic")]
    QishuiMusic,
    #[serde(rename = "other")]
    Other,
}

/// 诊断页面使用的会话来源标识及其识别结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaSessionIdentity {
    session_key: u64,
    source_app_id: String,
    player_kind: MediaPlayerKind,
}

/// 一次缩略图读取产生的前端图片和候选主色。
struct MediaArtwork {
    data_url: String,
    accent_color: Option<String>,
}

/// 用于统计相近颜色出现次数及其实际 RGB 均值。
#[derive(Clone, Copy, Default)]
struct ColorBucket {
    count: u32,
    red_sum: u64,
    green_sum: u64,
    blue_sum: u64,
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

/// 前端消费的统一媒体快照，汇总当前会话的显示信息、状态、能力和时间轴。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaSnapshot {
    session_key: u64,
    source_app_id: String,
    player_kind: MediaPlayerKind,
    title: String,
    artist: String,
    artwork_data_url: Option<String>,
    accent_color: String,
    system_accent_color: String,
    playback_status: CurrentPlaybackStatus,
    capabilities: CurrentPlaybackCapabilities,
    timeline: Option<CurrentTimeline>,
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

                if let Err(error) =
                    app.emit(CURRENT_MEDIA_METADATA_CHANGED_EVENT, Some(metadata.clone()))
                {
                    log::warn!("无法广播当前媒体会话元数据：{error}");
                }
                emit_media_snapshot(&request.session, &app, metadata);
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
                if let Err(emit_error) = app.emit(
                    CURRENT_MEDIA_SNAPSHOT_CHANGED_EVENT,
                    Option::<MediaSnapshot>::None,
                ) {
                    log::warn!("无法广播媒体快照读取失败：{emit_error}");
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
    media_activity_tracker: Option<MediaActivityTracker>,
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
        let media_activity_tracker = manager.as_ref().and_then(|manager| {
            MediaActivityTracker::start(manager, app)
                .map_err(|error| log::error!("无法启动媒体活动跟踪：{error}"))
                .ok()
        });
        let sessions_changed_token = manager.as_ref().and_then(|manager| {
            subscribe_to_sessions_changed(
                manager,
                app,
                Arc::clone(&current_session_observation),
                Arc::clone(&media_metadata_loader),
                media_activity_tracker.clone(),
            )
            .ok()
        });
        let current_session_changed_token = manager.as_ref().and_then(|manager| {
            subscribe_to_current_session_changed(
                manager,
                app,
                Arc::clone(&current_session_observation),
                Arc::clone(&media_metadata_loader),
                media_activity_tracker.clone(),
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
            if let Some(activity_tracker) = &media_activity_tracker {
                let current_session = manager.GetCurrentSession().ok();
                activity_tracker.mark_current_session(current_session.as_ref());
            }
        }

        Self {
            manager,
            sessions_changed_token,
            current_session_changed_token,
            current_session_observation,
            media_metadata_loader,
            media_activity_tracker,
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

    /// 枚举全部媒体会话，并返回 Muse Bar 对每个来源的播放器分类。
    pub(crate) fn session_identities(&self) -> Result<Vec<MediaSessionIdentity>, String> {
        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| "Windows 全局系统媒体管理器尚未初始化".to_owned())?;

        collect_session_identities(manager)
    }

    /// 返回全部媒体会话当前记录的有效活动时间和原因。
    pub(crate) fn session_activities(&self) -> Result<Vec<MediaSessionActivity>, String> {
        self.media_activity_tracker
            .as_ref()
            .ok_or_else(|| "媒体活动跟踪器尚未初始化".to_owned())?
            .activities()
    }

    /// 从内存缓存读取标题、歌手和封面，不在 Tauri 命令线程中等待 WinRT。
    pub(crate) fn current_media_metadata(&self) -> Result<Option<CurrentMediaMetadata>, String> {
        self.media_metadata_loader.cached()
    }

    /// 读取 Windows 当前会话的播放状态；没有当前会话时返回空值。
    pub(crate) fn current_playback_status(&self) -> Result<Option<CurrentPlaybackStatus>, String> {
        let Some(session) = self.observed_session()? else {
            return Ok(None);
        };

        read_playback_status(&session).map(Some)
    }

    /// 读取 Windows 当前会话声明的控制能力；没有当前会话时返回空值。
    pub(crate) fn current_playback_capabilities(
        &self,
    ) -> Result<Option<CurrentPlaybackCapabilities>, String> {
        let Some(session) = self.observed_session()? else {
            return Ok(None);
        };

        read_playback_capabilities(&session).map(Some)
    }

    /// 读取 Windows 当前会话的有效时间轴；没有会话或时间轴无效时返回空值。
    pub(crate) fn current_timeline(&self) -> Result<Option<CurrentTimeline>, String> {
        let Some(session) = self.observed_session()? else {
            return Ok(None);
        };

        read_timeline(&session)
    }

    /// 将当前会话与后台缓存的元数据组合为统一快照。
    pub(crate) fn current_media_snapshot(&self) -> Result<Option<MediaSnapshot>, String> {
        let Some(session) = self.observed_session()? else {
            return Ok(None);
        };
        let Some(metadata) = self.media_metadata_loader.cached()? else {
            return Ok(None);
        };

        if !metadata_belongs_to_session(&metadata, &session)? {
            return Ok(None);
        }

        compose_media_snapshot(&session, metadata).map(Some)
    }

    /// 根据活动记录重新选择 Bar 实际观察的会话，并在目标变化时重新绑定事件。
    pub(crate) fn refresh_selected_media_session<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<Option<SelectedMediaSession>, String> {
        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| "Windows 全局系统媒体管理器尚未初始化".to_owned())?;
        let activity_tracker = self
            .media_activity_tracker
            .as_ref()
            .ok_or_else(|| "媒体活动跟踪器尚未初始化".to_owned())?;
        select_and_bind_media_session(
            manager,
            activity_tracker,
            app,
            &self.current_session_observation,
            &self.media_metadata_loader,
        )
    }

    /// 对 Bar 当前真正观察的会话执行播放、暂停、切歌或进度跳转。
    pub(crate) fn control_media(&self, action: ControlAction) -> Result<(), MediaControlError> {
        let session = self
            .observed_session()
            .map_err(|message| MediaControlError::windows_api(action, message))?;
        execute_control_action(session.as_ref(), action)
    }

    /// 返回 Bar 当前真正观察的会话，而不是 Windows 自行选出的 CurrentSession。
    fn observed_session(
        &self,
    ) -> Result<Option<GlobalSystemMediaTransportControlsSession>, String> {
        self.current_session_observation
            .lock()
            .map(|observation| observation.session.clone())
            .map_err(|_| "当前媒体会话监听状态锁已损坏".to_owned())
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

/// 按最新活动记录选择观察会话，并在目标变化时完成事件解绑和重新绑定。
fn select_and_bind_media_session<R: Runtime>(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    activity_tracker: &MediaActivityTracker,
    app: &AppHandle<R>,
    observation: &Arc<Mutex<CurrentSessionObservation>>,
    media_metadata_loader: &Arc<MediaMetadataLoader>,
) -> Result<Option<SelectedMediaSession>, String> {
    let windows_current_session = manager.GetCurrentSession().ok();
    let (selected_session, selection) = activity_tracker.select_session(windows_current_session)?;
    let selected_key = selected_session.as_ref().map(session_key);
    let observed_key = observation
        .lock()
        .map(|observation| observation.session.as_ref().map(session_key))
        .map_err(|_| "当前媒体会话监听状态锁已损坏".to_owned())?;

    if selected_key != observed_key {
        bind_media_session(selected_session, app, observation, media_metadata_loader)?;
    }

    Ok(selection)
}

/// 注册会话列表变化事件，并将最新 Source App ID 广播给所有前端窗口。
fn subscribe_to_sessions_changed<R: Runtime>(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    app: &AppHandle<R>,
    observation: Arc<Mutex<CurrentSessionObservation>>,
    media_metadata_loader: Arc<MediaMetadataLoader>,
    media_activity_tracker: Option<MediaActivityTracker>,
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

            if let Some(activity_tracker) = &media_activity_tracker {
                if let Err(error) = activity_tracker.refresh_sessions(manager, &app) {
                    log::warn!("媒体会话列表变化后刷新活动监听失败：{error}");
                } else if let Err(error) = select_and_bind_media_session(
                    manager,
                    activity_tracker,
                    &app,
                    &observation,
                    &media_metadata_loader,
                ) {
                    log::warn!("媒体会话列表变化后重新选择观察会话失败：{error}");
                }
            }

            match collect_session_identities(manager) {
                Ok(identities) => {
                    let source_app_ids = identities
                        .iter()
                        .map(|identity| identity.source_app_id.clone())
                        .collect::<Vec<_>>();
                    if let Err(error) = app.emit(MEDIA_SESSIONS_CHANGED_EVENT, source_app_ids) {
                        log::warn!("无法广播系统媒体会话列表变化：{error}");
                    }
                    if let Err(error) = app.emit(MEDIA_SESSION_IDENTITIES_CHANGED_EVENT, identities)
                    {
                        log::warn!("无法广播媒体会话身份变化：{error}");
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
    media_activity_tracker: Option<MediaActivityTracker>,
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

            if let Some(activity_tracker) = &media_activity_tracker {
                let current_session = manager.GetCurrentSession().ok();
                activity_tracker.mark_current_session(current_session.as_ref());
            } else if let Err(error) =
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
    bind_media_session(current_session, app, observation, media_metadata_loader)
}

/// 注销旧会话并绑定选择器指定的媒体会话。
fn bind_media_session<R: Runtime>(
    current_session: Option<GlobalSystemMediaTransportControlsSession>,
    app: &AppHandle<R>,
    observation: &Arc<Mutex<CurrentSessionObservation>>,
    media_metadata_loader: &Arc<MediaMetadataLoader>,
) -> Result<(), String> {
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
        app.emit(
            CURRENT_MEDIA_SNAPSHOT_CHANGED_EVENT,
            Option::<MediaSnapshot>::None,
        )
        .map_err(|error| format!("无法广播当前媒体快照已清空：{error}"))?;
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
    let playback_metadata_loader = Arc::clone(media_metadata_loader);
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
            emit_media_snapshot_from_cache(session, &playback_app, &playback_metadata_loader);
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

/// 使用指定元数据构建并广播统一快照；单项读取失败不会影响原有分项事件。
fn emit_media_snapshot<R: Runtime>(
    session: &GlobalSystemMediaTransportControlsSession,
    app: &AppHandle<R>,
    metadata: CurrentMediaMetadata,
) {
    match compose_media_snapshot(session, metadata) {
        Ok(snapshot) => {
            if let Err(error) = app.emit(CURRENT_MEDIA_SNAPSHOT_CHANGED_EVENT, Some(snapshot)) {
                log::warn!("无法广播当前媒体统一快照：{error}");
            }
        }
        Err(error) => log::warn!("无法组装当前媒体统一快照：{error}"),
    }
}

/// 从后台元数据缓存构建统一快照，缓存仍属于旧会话时跳过本次广播。
fn emit_media_snapshot_from_cache<R: Runtime>(
    session: &GlobalSystemMediaTransportControlsSession,
    app: &AppHandle<R>,
    media_metadata_loader: &MediaMetadataLoader,
) {
    let metadata = match media_metadata_loader.cached() {
        Ok(Some(metadata)) => metadata,
        Ok(None) => return,
        Err(error) => {
            log::warn!("无法读取媒体元数据缓存以组装快照：{error}");
            return;
        }
    };

    match metadata_belongs_to_session(&metadata, session) {
        Ok(true) => emit_media_snapshot(session, app, metadata),
        Ok(false) => {}
        Err(error) => log::warn!("无法确认媒体快照所属会话：{error}"),
    }
}

/// 检查缓存元数据是否仍属于指定会话，避免切换播放器时组合出交叉数据。
fn metadata_belongs_to_session(
    metadata: &CurrentMediaMetadata,
    session: &GlobalSystemMediaTransportControlsSession,
) -> Result<bool, String> {
    let source_app_id = session
        .SourceAppUserModelId()
        .map_err(|error| format!("无法核对媒体快照的 Source App ID：{error}"))?;
    Ok(metadata.source_app_id == source_app_id)
}

/// 从同一会话读取播放状态、能力和时间轴，并与元数据合并。
fn compose_media_snapshot(
    session: &GlobalSystemMediaTransportControlsSession,
    metadata: CurrentMediaMetadata,
) -> Result<MediaSnapshot, String> {
    let playback_info = session
        .GetPlaybackInfo()
        .map_err(|error| format!("无法为媒体快照读取播放信息：{error}"))?;
    let playback_status = playback_status_from_info(&playback_info)?;
    let capabilities = playback_capabilities_from_info(&playback_info)?;
    let timeline = read_timeline(session)?;

    Ok(MediaSnapshot {
        session_key: session_key(session),
        player_kind: identify_media_player(&metadata.source_app_id),
        source_app_id: metadata.source_app_id,
        title: metadata.title,
        artist: metadata.artist,
        artwork_data_url: metadata.artwork_data_url,
        accent_color: metadata.accent_color,
        system_accent_color: read_windows_accent_color(),
        playback_status,
        capabilities,
        timeline,
    })
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
    let artwork = match properties.Thumbnail() {
        Ok(thumbnail) => match read_artwork(&thumbnail) {
            Ok(artwork) => artwork,
            Err(error) => {
                // 个别播放器会发布暂时不可读的缩略图。封面失败不应丢弃同批标题和歌手。
                log::warn!("无法读取当前会话封面，本次使用空封面：{error}");
                None
            }
        },
        Err(_) => None,
    };
    let accent_color = artwork
        .as_ref()
        .and_then(|artwork| artwork.accent_color.clone())
        .unwrap_or_else(read_windows_accent_color);
    let artwork_data_url = artwork.map(|artwork| artwork.data_url);

    Ok(CurrentMediaMetadata {
        source_app_id: source_app_id.to_string(),
        title: title.to_string(),
        artist: artist.to_string(),
        artwork_data_url,
        accent_color,
    })
}

/// 读取一次 SMTC 缩略图流，同时生成 WebView 图片和封面主色。
fn read_artwork(thumbnail: &IRandomAccessStreamReference) -> Result<Option<MediaArtwork>, String> {
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
    // 当前函数运行在专用媒体元数据线程中，图片解码不会阻塞 Tauri 命令或 WebView。
    let accent_color = extract_dominant_color(&bytes);
    let encoded = BASE64_STANDARD.encode(&bytes);

    Ok(Some(MediaArtwork {
        data_url: format!("data:{content_type};base64,{encoded}"),
        accent_color,
    }))
}

/// 从缩小后的封面中选择出现频率高、且在深浅背景上都可辨识的颜色。
fn extract_dominant_color(bytes: &[u8]) -> Option<String> {
    let image = image::load_from_memory(bytes)
        .ok()?
        .thumbnail(48, 48)
        .to_rgba8();
    let mut buckets = [ColorBucket::default(); 4096];

    for pixel in image.pixels() {
        let [red, green, blue, alpha] = pixel.0;
        if alpha < 128 {
            continue;
        }

        let index = ((usize::from(red) >> 4) << 8)
            | ((usize::from(green) >> 4) << 4)
            | (usize::from(blue) >> 4);
        let bucket = &mut buckets[index];
        bucket.count += 1;
        bucket.red_sum += u64::from(red);
        bucket.green_sum += u64::from(green);
        bucket.blue_sum += u64::from(blue);
    }

    let mut best: Option<(u64, u8, u8, u8)> = None;
    for bucket in buckets.into_iter().filter(|bucket| bucket.count > 0) {
        let count = u64::from(bucket.count);
        let red = (bucket.red_sum / count) as u8;
        let green = (bucket.green_sum / count) as u8;
        let blue = (bucket.blue_sum / count) as u8;
        let maximum = red.max(green).max(blue);
        let minimum = red.min(green).min(blue);
        let saturation = maximum - minimum;

        // 低饱和颜色容易与任务栏融为一体；同时要求候选色在典型深浅背景上均有辨识度。
        if saturation < 24 || !has_sufficient_taskbar_contrast(red, green, blue) {
            continue;
        }

        // 在出现频率仍占主导的前提下，提高鲜艳候选色的权重，避免总是选中大面积灰暗背景。
        let score = count * (u64::from(saturation) * 2 + 32);
        if best
            .as_ref()
            .map_or(true, |(best_score, ..)| score > *best_score)
        {
            best = Some((score, red, green, blue));
        }
    }

    best.map(|(_, red, green, blue)| {
        let (red, green, blue) = enhance_color_saturation(red, green, blue);
        format!("#{red:02X}{green:02X}{blue:02X}")
    })
}

/// 温和提高颜色饱和度并保持最高通道亮度；增强后对比度不足时保留原色。
fn enhance_color_saturation(red: u8, green: u8, blue: u8) -> (u8, u8, u8) {
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let difference = maximum - minimum;
    if maximum == 0 || difference == 0 {
        return (red, green, blue);
    }

    let maximum = f64::from(maximum);
    let saturation = f64::from(difference) / maximum;
    let enhanced_saturation = (saturation * 1.15 + 0.05).min(1.0);
    let expansion = enhanced_saturation / saturation;

    // 以最亮通道为锚点向外拉开其余通道，保持色相和视觉亮度基本稳定。
    let enhance_channel = |channel: u8| {
        (maximum - (maximum - f64::from(channel)) * expansion)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    let enhanced = (
        enhance_channel(red),
        enhance_channel(green),
        enhance_channel(blue),
    );

    if has_sufficient_taskbar_contrast(enhanced.0, enhanced.1, enhanced.2) {
        enhanced
    } else {
        (red, green, blue)
    }
}

/// 判断颜色相对典型深色与浅色任务栏背景是否都具有最低辨识度。
fn has_sufficient_taskbar_contrast(red: u8, green: u8, blue: u8) -> bool {
    let luminance = relative_luminance(red, green, blue);
    let light_background = relative_luminance(245, 245, 245);
    let dark_background = relative_luminance(32, 32, 32);
    contrast_ratio(luminance, light_background) >= 1.6
        && contrast_ratio(luminance, dark_background) >= 1.6
}

/// 将 sRGB 颜色转换为用于对比度计算的相对亮度。
fn relative_luminance(red: u8, green: u8, blue: u8) -> f64 {
    /// 将单个 sRGB 通道转换为线性光通道。
    fn linearize(channel: u8) -> f64 {
        let channel = f64::from(channel) / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * linearize(red) + 0.7152 * linearize(green) + 0.0722 * linearize(blue)
}

/// 计算两个相对亮度之间的 WCAG 对比度。
fn contrast_ratio(first: f64, second: f64) -> f64 {
    let lighter = first.max(second);
    let darker = first.min(second);
    (lighter + 0.05) / (darker + 0.05)
}

/// 读取 Windows 当前强调色；系统 API 不可用时采用 Windows 默认蓝色。
fn read_windows_accent_color() -> String {
    let mut colorization = 0_u32;
    let mut opaque_blend = BOOL::default();
    if unsafe { DwmGetColorizationColor(&mut colorization, &mut opaque_blend) }.is_err() {
        return DEFAULT_WINDOWS_ACCENT_COLOR.to_owned();
    }

    format!("#{:06X}", colorization & 0x00FF_FFFF)
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
    collect_session_identities(manager).map(|identities| {
        identities
            .into_iter()
            .map(|identity| identity.source_app_id)
            .collect()
    })
}

/// 从指定管理器读取全部会话，并为每个 Source App ID 添加播放器类别。
fn collect_session_identities(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
) -> Result<Vec<MediaSessionIdentity>, String> {
    let sessions = manager
        .GetSessions()
        .map_err(|error| format!("无法枚举系统媒体会话：{error}"))?;
    let session_count = sessions
        .Size()
        .map_err(|error| format!("无法读取系统媒体会话数量：{error}"))?;
    let mut identities = Vec::with_capacity(session_count as usize);

    for index in 0..session_count {
        let session = sessions
            .GetAt(index)
            .map_err(|error| format!("无法读取第 {} 个系统媒体会话：{error}", index + 1))?;
        let source_app_id = session
            .SourceAppUserModelId()
            .map_err(|error| format!("无法读取第 {} 个会话的 Source App ID：{error}", index + 1))?;
        let source_app_id = source_app_id.to_string();
        identities.push(MediaSessionIdentity {
            session_key: session_key(&session),
            player_kind: identify_media_player(&source_app_id),
            source_app_id,
        });
    }

    Ok(identities)
}

/// 根据 Source App ID 中稳定的品牌或可执行文件标识识别四个目标播放器。
pub(crate) fn identify_media_player(source_app_id: &str) -> MediaPlayerKind {
    let normalized = source_app_id.to_ascii_lowercase();

    if normalized.contains("qqmusic") {
        MediaPlayerKind::QqMusic
    } else if normalized.contains("cloudmusic") || normalized.contains("netease") {
        MediaPlayerKind::NeteaseCloudMusic
    } else if normalized.contains("kugou") || normalized.contains("kgmusic") {
        MediaPlayerKind::KugouMusic
    } else if normalized == "汽水音乐"
        || normalized.contains("qishui")
        || normalized.contains("com.ss.android.ugc.luna")
        || normalized.ends_with("luna.exe")
    {
        MediaPlayerKind::QishuiMusic
    } else {
        MediaPlayerKind::Other
    }
}
