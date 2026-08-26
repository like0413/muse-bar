use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::background_worker::{join_with_timeout, WORKER_SHUTDOWN_TIMEOUT};

use super::{
    activity::{session_key, MediaActivityTracker, MediaSessionActivity, SelectedMediaSession},
    artwork::{read_artwork, read_windows_accent_color},
    control::{execute_control_action, ControlAction, MediaControlError},
    model::{
        bounded_media_text, identify_media_player, CurrentMediaMetadata,
        CurrentPlaybackCapabilities, CurrentPlaybackState, CurrentPlaybackStatus, CurrentTimeline,
        MediaSessionIdentity, MediaSnapshot,
    },
};
use tauri::{AppHandle, Emitter, Runtime};
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackInfo,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus, MediaPropertiesChangedEventArgs,
    PlaybackInfoChangedEventArgs, SessionsChangedEventArgs, TimelinePropertiesChangedEventArgs,
};
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

const TICKS_PER_MILLISECOND: i64 = 10_000;
const WINDOWS_TO_UNIX_EPOCH_TICKS: i64 = 116_444_736_000_000_000;
const MEDIA_SESSION_IDENTITIES_CHANGED_EVENT: &str = "media-session-identities-changed";
const CURRENT_TIMELINE_CHANGED_EVENT: &str = "current-timeline-changed";
const CURRENT_PLAYBACK_STATE_CHANGED_EVENT: &str = "current-playback-state-changed";
const CURRENT_MEDIA_SNAPSHOT_CHANGED_EVENT: &str = "current-media-snapshot-changed";
const MEDIA_RUNTIME_RETRY_INTERVAL: Duration = Duration::from_secs(2);

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

/// 只保留最新一次元数据请求的有界邮箱。
#[derive(Default)]
struct MediaMetadataMailbox {
    pending: Option<MediaMetadataRequest>,
    shutdown: bool,
}

/// 保存异步媒体属性任务队列、最近快照和版本号。
struct MediaMetadataLoader {
    mailbox: Arc<(Mutex<MediaMetadataMailbox>, Condvar)>,
    cached: Arc<Mutex<Option<CurrentMediaMetadata>>>,
    revision: Arc<AtomicU64>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl MediaMetadataLoader {
    /// 创建唯一的 MTA 工作线程，让不可跨线程的 WinRT 流始终留在同一 apartment。
    fn start<R: Runtime>(app: &AppHandle<R>) -> Result<Self, String> {
        let mailbox = Arc::new((Mutex::new(MediaMetadataMailbox::default()), Condvar::new()));
        let cached = Arc::new(Mutex::new(None));
        let revision = Arc::new(AtomicU64::new(0));
        let worker_app = app.clone();
        let worker_cached = Arc::clone(&cached);
        let worker_revision = Arc::clone(&revision);
        let worker_mailbox = Arc::clone(&mailbox);

        let worker = thread::Builder::new()
            .name("muse-bar-media-metadata".to_owned())
            .spawn(move || {
                run_media_metadata_worker(
                    worker_mailbox,
                    worker_cached,
                    worker_revision,
                    worker_app,
                );
            })
            .map_err(|error| format!("无法启动媒体元数据异步线程：{error}"))?;

        Ok(Self {
            mailbox,
            cached,
            revision,
            worker: Mutex::new(Some(worker)),
        })
    }

    /// 提交一次完整元数据读取；这里只入队，不等待任何 WinRT 操作。
    fn request(&self, session: &GlobalSystemMediaTransportControlsSession) {
        let revision = self.revision.fetch_add(1, Ordering::AcqRel) + 1;
        let request = MediaMetadataRequest {
            session: session.clone(),
            revision,
        };

        let (mailbox, ready) = &*self.mailbox;
        match mailbox.lock() {
            Ok(mut mailbox) if !mailbox.shutdown => {
                mailbox.pending = Some(request);
                ready.notify_one();
            }
            Ok(_) => log::warn!("媒体元数据线程正在关闭，已忽略新的读取请求"),
            Err(_) => log::warn!("无法提交媒体元数据读取任务：任务邮箱锁已损坏"),
        }
    }

    /// 当前会话消失时使在途任务失效，并清空完整元数据缓存。
    fn clear(&self) -> Result<(), String> {
        self.revision.fetch_add(1, Ordering::AcqRel);
        let (mailbox, _) = &*self.mailbox;
        mailbox
            .lock()
            .map_err(|_| "媒体元数据任务邮箱锁已损坏".to_owned())?
            .pending = None;
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

impl Drop for MediaMetadataLoader {
    fn drop(&mut self) {
        let (mailbox, ready) = &*self.mailbox;
        if let Ok(mut mailbox) = mailbox.lock() {
            mailbox.shutdown = true;
            mailbox.pending = None;
            ready.notify_one();
        }
        if let Ok(worker) = self.worker.get_mut() {
            if let Some(worker) = worker.take() {
                join_with_timeout(worker, "媒体元数据", WORKER_SHUTDOWN_TIMEOUT);
            }
        }
    }
}

/// 在固定 MTA apartment 内异步读取完整 MediaProperties，并丢弃过期切歌结果。
fn run_media_metadata_worker<R: Runtime>(
    mailbox: Arc<(Mutex<MediaMetadataMailbox>, Condvar)>,
    cached: Arc<Mutex<Option<CurrentMediaMetadata>>>,
    revision: Arc<AtomicU64>,
    app: AppHandle<R>,
) {
    // 原始线程没有 COM apartment；WinRT 媒体属性与缩略图流必须在初始化后使用。
    if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
        log::error!("无法初始化媒体元数据线程的 WinRT 环境：{error}");
        return;
    }

    loop {
        let request = {
            let (state, ready) = &*mailbox;
            let mut state = match state.lock() {
                Ok(state) => state,
                Err(_) => {
                    log::error!("媒体元数据线程已停止：任务邮箱锁已损坏");
                    break;
                }
            };
            while state.pending.is_none() && !state.shutdown {
                state = match ready.wait(state) {
                    Ok(state) => state,
                    Err(_) => {
                        log::error!("媒体元数据线程已停止：等待任务时邮箱锁已损坏");
                        return unsafe { RoUninitialize() };
                    }
                };
            }
            if state.shutdown {
                break;
            }
            let Some(request) = state.pending.take() else {
                continue;
            };
            request
        };

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

                emit_media_snapshot(&request.session, &app, metadata);
            }
            Err(error) => {
                if let Ok(mut cached) = cached.lock() {
                    *cached = None;
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

/// 允许首次 WinRT 初始化失败后在下一次媒体 IPC 时重新创建完整运行时。
pub(crate) struct SystemMediaManager {
    runtime: Mutex<MediaRuntimeSlot>,
}

struct MediaRuntimeSlot {
    runtime: Option<Arc<SystemMediaRuntime>>,
    retry_after: Option<Instant>,
}

/// 一次完整初始化成功后共同生效的管理器、事件订阅、观察器和后台线程。
struct SystemMediaRuntime {
    manager: GlobalSystemMediaTransportControlsSessionManager,
    sessions_changed_token: i64,
    current_session_changed_token: i64,
    current_session_observation: Arc<Mutex<CurrentSessionObservation>>,
    media_metadata_loader: Arc<MediaMetadataLoader>,
    media_activity_tracker: MediaActivityTracker,
}

impl SystemMediaManager {
    /// 尝试初始化媒体运行时；失败不阻止应用启动，后续 IPC 会继续重试。
    pub(crate) fn initialize<R: Runtime>(app: &AppHandle<R>) -> Self {
        let runtime = SystemMediaRuntime::initialize(app)
            .inspect_err(|error| log::error!("无法初始化媒体运行时，等待后续重试：{error}"))
            .ok()
            .map(Arc::new);
        Self {
            runtime: Mutex::new(MediaRuntimeSlot {
                retry_after: runtime
                    .is_none()
                    .then(|| Instant::now() + MEDIA_RUNTIME_RETRY_INTERVAL),
                runtime,
            }),
        }
    }

    /// 注销媒体事件并停止后台线程；重复调用保持幂等。
    pub(crate) fn request_shutdown(&self) {
        match self.runtime.lock() {
            Ok(mut slot) => {
                slot.runtime = None;
                slot.retry_after = Some(Instant::now() + MEDIA_RUNTIME_RETRY_INTERVAL);
            }
            Err(_) => log::warn!("应用退出时无法取得媒体运行时状态锁"),
        }
    }

    fn ensure_runtime<R: Runtime>(
        slot: &mut MediaRuntimeSlot,
        app: &AppHandle<R>,
    ) -> Result<Arc<SystemMediaRuntime>, String> {
        if slot.runtime.is_none() {
            let now = Instant::now();
            if !media_runtime_retry_is_due(slot.retry_after, now) {
                return Err("Windows 媒体服务暂时不可用，正在等待自动重试".to_owned());
            }
            match SystemMediaRuntime::initialize(app) {
                Ok(runtime) => {
                    slot.runtime = Some(Arc::new(runtime));
                    slot.retry_after = None;
                }
                Err(error) => {
                    slot.retry_after = Some(now + MEDIA_RUNTIME_RETRY_INTERVAL);
                    return Err(error);
                }
            }
        }
        slot.runtime
            .as_ref()
            .map(Arc::clone)
            .ok_or_else(|| "媒体运行时尚未初始化".to_owned())
    }

    /// 在完整运行时上执行操作；运行时缺失时先进行一次事务式重建。
    fn with_runtime<R: Runtime, T>(
        &self,
        app: &AppHandle<R>,
        operation: impl FnOnce(&SystemMediaRuntime) -> Result<T, String>,
    ) -> Result<T, String> {
        let runtime = {
            let mut slot = self
                .runtime
                .lock()
                .map_err(|_| "媒体运行时状态锁已损坏".to_owned())?;
            Self::ensure_runtime(&mut slot, app)?
        };
        let result = operation(&runtime);
        if result.is_err() {
            let mut slot = self
                .runtime
                .lock()
                .map_err(|_| "媒体运行时状态锁已损坏".to_owned())?;
            if slot
                .runtime
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &runtime))
            {
                slot.runtime = None;
                slot.retry_after = Some(Instant::now() + MEDIA_RUNTIME_RETRY_INTERVAL);
            }
        }
        result
    }

    /// 枚举全部媒体会话，并返回 Muse Bar 对每个来源的播放器分类。
    pub(crate) fn session_identities<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<Vec<MediaSessionIdentity>, String> {
        self.with_runtime(app, |runtime| collect_session_identities(&runtime.manager))
    }

    /// 返回全部媒体会话当前记录的有效活动时间和原因。
    pub(crate) fn session_activities<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<Vec<MediaSessionActivity>, String> {
        self.with_runtime(app, |runtime| runtime.media_activity_tracker.activities())
    }

    /// 将当前会话与后台缓存的元数据组合为统一快照。
    pub(crate) fn current_media_snapshot<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<Option<MediaSnapshot>, String> {
        self.with_runtime(app, SystemMediaRuntime::current_media_snapshot)
    }

    /// 根据活动记录重新选择 Bar 实际观察的会话，并在目标变化时重新绑定事件。
    pub(crate) fn refresh_selected_media_session<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<Option<SelectedMediaSession>, String> {
        self.with_runtime(app, |runtime| runtime.refresh_selected_media_session(app))
    }

    /// 对 Bar 当前真正观察的会话执行播放、暂停、切歌或进度跳转。
    pub(crate) fn control_media<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        action: ControlAction,
    ) -> Result<(), MediaControlError> {
        let runtime = {
            let mut slot = self
                .runtime
                .lock()
                .map_err(|_| MediaControlError::windows_api(action, "媒体运行时状态锁已损坏"))?;
            Self::ensure_runtime(&mut slot, app)
                .map_err(|error| MediaControlError::windows_api(action, error))?
        };
        runtime.control_media(action)
    }
}

fn media_runtime_retry_is_due(retry_after: Option<Instant>, now: Instant) -> bool {
    retry_after.map_or(true, |retry_after| now >= retry_after)
}

impl SystemMediaRuntime {
    /// 完成管理器、活动跟踪、两个全局事件和当前会话的事务式初始化。
    fn initialize<R: Runtime>(app: &AppHandle<R>) -> Result<Self, String> {
        let current_session_observation =
            Arc::new(Mutex::new(CurrentSessionObservation::default()));
        let media_metadata_loader = Arc::new(MediaMetadataLoader::start(app)?);
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .and_then(|operation| operation.get())
            .map_err(|error| format!("无法初始化 Windows 全局系统媒体管理器：{error}"))?;
        let media_activity_tracker = MediaActivityTracker::start(&manager, app)?;
        let sessions_changed_token = subscribe_to_sessions_changed(
            &manager,
            app,
            Arc::clone(&current_session_observation),
            Arc::clone(&media_metadata_loader),
            Some(media_activity_tracker.clone()),
        )?;
        let current_session_changed_token = match subscribe_to_current_session_changed(
            &manager,
            app,
            Arc::clone(&current_session_observation),
            Arc::clone(&media_metadata_loader),
            Some(media_activity_tracker.clone()),
        ) {
            Ok(token) => token,
            Err(error) => {
                let _ = manager.RemoveSessionsChanged(sessions_changed_token);
                return Err(error);
            }
        };

        if let Err(error) = bind_current_session(
            &manager,
            app,
            &current_session_observation,
            &media_metadata_loader,
        ) {
            log::warn!("媒体运行时已启动，但初始会话绑定失败，将由后续刷新恢复：{error}");
        }
        let current_session = manager.GetCurrentSession().ok();
        media_activity_tracker.mark_current_session(current_session.as_ref());

        Ok(Self {
            manager,
            sessions_changed_token,
            current_session_changed_token,
            current_session_observation,
            media_metadata_loader,
            media_activity_tracker,
        })
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
        select_and_bind_media_session(
            &self.manager,
            &self.media_activity_tracker,
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

impl Drop for SystemMediaRuntime {
    /// 应用退出时注销 WinRT 事件，避免管理器继续持有已无用途的回调。
    fn drop(&mut self) {
        if let Err(error) = self
            .manager
            .RemoveSessionsChanged(self.sessions_changed_token)
        {
            log::warn!("无法注销系统媒体会话列表监听：{error}");
        }
        if let Err(error) = self
            .manager
            .RemoveCurrentSessionChanged(self.current_session_changed_token)
        {
            log::warn!("无法注销 Windows 当前媒体会话监听：{error}");
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
    } else if should_retry_media_metadata(
        selected_key,
        observed_key,
        media_metadata_loader.cached()?.is_some(),
    ) {
        if let Some(session) = selected_session.as_ref() {
            media_metadata_loader.request(session);
        }
    }

    Ok(selection)
}

/// 同一会话已绑定但元数据仍为空时重新请求，避免冷启动瞬时失败后永久保持空快照。
fn should_retry_media_metadata(
    selected_key: Option<u64>,
    observed_key: Option<u64>,
    has_cached_metadata: bool,
) -> bool {
    selected_key.is_some() && selected_key == observed_key && !has_cached_metadata
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
                    if let Err(error) = app.emit_to(
                        "settings",
                        MEDIA_SESSION_IDENTITIES_CHANGED_EVENT,
                        identities,
                    ) {
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
    let playback_handler: TypedEventHandler<
        GlobalSystemMediaTransportControlsSession,
        PlaybackInfoChangedEventArgs,
    > = TypedEventHandler::new(
        move |sender: windows::core::Ref<'_, GlobalSystemMediaTransportControlsSession>,
              _: windows::core::Ref<'_, PlaybackInfoChangedEventArgs>| {
            let Some(session) = sender.as_ref() else {
                return Ok(());
            };

            emit_timeline(session, &playback_app);
            emit_playback_state(session, &playback_app);
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

/// 广播不含封面的轻量播放状态，避免播放/暂停时重复传输完整媒体快照。
fn emit_playback_state<R: Runtime>(
    session: &GlobalSystemMediaTransportControlsSession,
    app: &AppHandle<R>,
) {
    let state = (|| {
        let playback_info = session
            .GetPlaybackInfo()
            .map_err(|error| format!("无法读取播放状态事件：{error}"))?;
        Ok::<_, String>(CurrentPlaybackState {
            session_key: session_key(session),
            playback_status: playback_status_from_info(&playback_info)?,
            capabilities: playback_capabilities_from_info(&playback_info)?,
        })
    })();

    match state {
        Ok(state) => {
            if let Err(error) = app.emit(CURRENT_PLAYBACK_STATE_CHANGED_EVENT, state) {
                log::warn!("无法广播当前播放状态：{error}");
            }
        }
        Err(error) => log::warn!("无法组装当前播放状态事件：{error}"),
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
        title: bounded_media_text(title.to_string()),
        artist: bounded_media_text(artist.to_string()),
        artwork_data_url,
        accent_color,
    })
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
