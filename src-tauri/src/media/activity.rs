use std::{
    collections::{HashMap, HashSet},
    sync::{
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use windows::{
    core::{IUnknown, Interface},
    Foundation::TypedEventHandler,
    Media::Control::{
        GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus, MediaPropertiesChangedEventArgs,
        PlaybackInfoChangedEventArgs,
    },
    Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
};

use crate::background_worker::{join_with_timeout, WORKER_SHUTDOWN_TIMEOUT};

use super::{
    model::{bounded_media_text, identify_media_player, MediaPlayerKind},
    selection::MediaSelectionCandidate,
};

const MEDIA_SESSION_ACTIVITIES_CHANGED_EVENT: &str = "media-session-activities-changed";
const MAX_PENDING_ACTIVITY_REQUESTS: usize = 64;

/// 最近一次有效活动发生的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MediaActivityReason {
    DetectedPlaying,
    PlaybackStarted,
    TrackChanged,
    BecameCurrent,
}

/// 统一选择器采用某个会话的原因。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MediaSelectionReason {
    PlayingPreferred,
    LastPausedPreferred,
    DetectedPreferred,
    WindowsCurrentFallback,
}

/// Rust 选择器返回给前端诊断的当前会话身份。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectedMediaSession {
    session_key: u64,
    source_app_id: String,
    player_kind: MediaPlayerKind,
    reason: MediaSelectionReason,
}

/// 设置页可观察的单个媒体会话活动状态。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaSessionActivity {
    session_key: u64,
    source_app_id: String,
    player_kind: MediaPlayerKind,
    title: Option<String>,
    artist: Option<String>,
    is_playing: bool,
    is_paused: bool,
    last_activity_at_unix_ms: Option<u64>,
    activity_sequence: Option<u64>,
    last_activity_reason: Option<MediaActivityReason>,
}

/// 后台线程使用的活动记录；标题和歌手用于区分真正切歌与重复属性通知。
struct ActivityRecord {
    source_app_id: String,
    player_kind: MediaPlayerKind,
    title: Option<String>,
    artist: Option<String>,
    is_playing: bool,
    is_paused: bool,
    last_activity_at_unix_ms: Option<u64>,
    activity_sequence: Option<u64>,
    last_activity_reason: Option<MediaActivityReason>,
}

/// 保存所有活动记录和严格递增的活动序号。
#[derive(Default)]
struct ActivityState {
    records: HashMap<u64, ActivityRecord>,
    next_activity_sequence: u64,
}

/// 需要在固定 WinRT apartment 中处理的活动变化。
enum ActivityRequestKind {
    Initialize,
    PlaybackChanged,
    MediaPropertiesChanged,
    BecameCurrent,
}

/// 将会话及其稳定进程内标识交给活动后台线程。
struct ActivityRequest {
    session_key: u64,
    session: GlobalSystemMediaTransportControlsSession,
    kind: ActivityRequestKind,
}

/// 活动线程接收的更新或显式停止消息。
enum ActivityMessage {
    Update(ActivityRequest),
    Shutdown,
}

/// 保存注销单个会话事件所需的 token。
struct SessionActivityObservation {
    session_key: u64,
    session: GlobalSystemMediaTransportControlsSession,
    playback_token: i64,
    media_properties_token: i64,
}

impl SessionActivityObservation {
    /// 注销当前会话的两个活动监听器。
    fn unsubscribe(&self) {
        if let Err(error) = self.session.RemovePlaybackInfoChanged(self.playback_token) {
            log::warn!("无法注销媒体活动播放监听：{error}");
        }
        if let Err(error) = self
            .session
            .RemoveMediaPropertiesChanged(self.media_properties_token)
        {
            log::warn!("无法注销媒体活动属性监听：{error}");
        }
    }
}

/// 仅在最后一个跟踪器引用释放时清理全部会话监听。
struct MediaActivityTrackerInner {
    sender: Option<SyncSender<ActivityMessage>>,
    state: Arc<Mutex<ActivityState>>,
    observations: Mutex<Vec<SessionActivityObservation>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for MediaActivityTrackerInner {
    /// 应用退出时注销所有会话监听，随后发送端释放会让后台线程自然结束。
    fn drop(&mut self) {
        if let Ok(observations) = self.observations.get_mut() {
            for observation in observations.iter() {
                observation.unsubscribe();
            }
        }
        if let Some(sender) = self.sender.take() {
            let _ = sender.try_send(ActivityMessage::Shutdown);
            drop(sender);
        }
        if let Ok(worker) = self.worker.get_mut() {
            if let Some(worker) = worker.take() {
                join_with_timeout(worker, "媒体活动", WORKER_SHUTDOWN_TIMEOUT);
            }
        }
    }
}

/// 跟踪所有系统媒体会话的有效活动时间。
#[derive(Clone)]
pub(crate) struct MediaActivityTracker {
    inner: Arc<MediaActivityTrackerInner>,
}

impl MediaActivityTracker {
    /// 创建活动后台线程，并立即监听当前已经存在的全部媒体会话。
    pub(crate) fn start<R: Runtime>(
        manager: &GlobalSystemMediaTransportControlsSessionManager,
        app: &AppHandle<R>,
    ) -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel(MAX_PENDING_ACTIVITY_REQUESTS);
        let state = Arc::new(Mutex::new(ActivityState::default()));
        let worker_state = Arc::clone(&state);
        let worker_app = app.clone();

        let worker = thread::Builder::new()
            .name("muse-bar-media-activity".to_owned())
            .spawn(move || run_activity_worker(receiver, worker_state, worker_app))
            .map_err(|error| format!("无法启动媒体活动后台线程：{error}"))?;

        let tracker = Self {
            inner: Arc::new(MediaActivityTrackerInner {
                sender: Some(sender),
                state,
                observations: Mutex::new(Vec::new()),
                worker: Mutex::new(Some(worker)),
            }),
        };
        tracker.refresh_sessions(manager, app)?;
        Ok(tracker)
    }

    /// 会话列表变化后重新绑定监听，并保留仍然存在会话的活动时间。
    pub(crate) fn refresh_sessions<R: Runtime>(
        &self,
        manager: &GlobalSystemMediaTransportControlsSessionManager,
        app: &AppHandle<R>,
    ) -> Result<(), String> {
        let sessions = manager
            .GetSessions()
            .map_err(|error| format!("无法为活动跟踪枚举媒体会话：{error}"))?;
        let session_count = sessions
            .Size()
            .map_err(|error| format!("无法读取活动跟踪会话数量：{error}"))?;
        let mut session_entries = Vec::with_capacity(session_count as usize);

        for index in 0..session_count {
            let session = sessions
                .GetAt(index)
                .map_err(|error| format!("无法读取第 {} 个活动跟踪会话：{error}", index + 1))?;
            let source_app_id = session
                .SourceAppUserModelId()
                .map_err(|error| format!("无法读取活动跟踪会话来源：{error}"))?
                .to_string();
            session_entries.push((session_key(&session), source_app_id, session));
        }

        let old_observations = {
            let mut observations = self
                .inner
                .observations
                .lock()
                .map_err(|_| "媒体活动监听状态锁已损坏".to_owned())?;
            std::mem::take(&mut *observations)
        };
        for observation in old_observations {
            observation.unsubscribe();
        }

        let active_keys = session_entries
            .iter()
            .map(|(session_key, ..)| *session_key)
            .collect::<HashSet<_>>();
        {
            let mut state = self
                .inner
                .state
                .lock()
                .map_err(|_| "媒体活动记录锁已损坏".to_owned())?;
            state
                .records
                .retain(|session_key, _| active_keys.contains(session_key));
            for (session_key, source_app_id, _) in &session_entries {
                state
                    .records
                    .entry(*session_key)
                    .or_insert_with(|| ActivityRecord {
                        player_kind: identify_media_player(source_app_id),
                        source_app_id: source_app_id.clone(),
                        title: None,
                        artist: None,
                        is_playing: false,
                        is_paused: false,
                        last_activity_at_unix_ms: None,
                        activity_sequence: None,
                        last_activity_reason: None,
                    });
            }
        }

        let mut new_observations = Vec::with_capacity(session_entries.len());
        let sender = self
            .inner
            .sender
            .as_ref()
            .ok_or_else(|| "媒体活动跟踪器正在关闭".to_owned())?;
        for (session_key, _, session) in session_entries {
            new_observations.push(subscribe_session_activity(session_key, &session, sender)?);
        }
        *self
            .inner
            .observations
            .lock()
            .map_err(|_| "媒体活动监听状态锁已损坏".to_owned())? = new_observations;

        emit_activity_state(&self.inner.state, app);
        Ok(())
    }

    /// 将 Windows 新选中的 CurrentSession 记录为一次有效活动。
    pub(crate) fn mark_current_session(
        &self,
        session: Option<&GlobalSystemMediaTransportControlsSession>,
    ) {
        let Some(session) = session else {
            return;
        };
        if let Some(sender) = &self.inner.sender {
            send_activity_request(
                sender,
                ActivityRequest {
                    session_key: session_key(session),
                    session: session.clone(),
                    kind: ActivityRequestKind::BecameCurrent,
                },
            );
        }
    }

    /// 返回全部会话当前的活动诊断快照。
    pub(crate) fn activities(&self) -> Result<Vec<MediaSessionActivity>, String> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| "媒体活动记录锁已损坏".to_owned())?;
        Ok(activity_snapshots(&state))
    }

    /// 按“正在播放、最近暂停、Windows 当前项”的顺序选择实际显示会话。
    pub(crate) fn select_session(
        &self,
        windows_current_session: Option<GlobalSystemMediaTransportControlsSession>,
    ) -> Result<
        (
            Option<GlobalSystemMediaTransportControlsSession>,
            Option<SelectedMediaSession>,
        ),
        String,
    > {
        let observations = self
            .inner
            .observations
            .lock()
            .map_err(|_| "媒体活动监听状态锁已损坏".to_owned())?;
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| "媒体活动记录锁已损坏".to_owned())?;

        let selected_key = select_preferred_session_key(&state);
        if let Some(selected_key) = selected_key {
            let selected_session = observations
                .iter()
                .find(|observation| observation.session_key == selected_key)
                .map(|observation| observation.session.clone());
            if let (Some(session), Some(record)) =
                (selected_session, state.records.get(&selected_key))
            {
                let reason = if record.is_playing {
                    MediaSelectionReason::PlayingPreferred
                } else {
                    MediaSelectionReason::LastPausedPreferred
                };
                return Ok((
                    Some(session),
                    Some(SelectedMediaSession {
                        session_key: selected_key,
                        source_app_id: record.source_app_id.clone(),
                        player_kind: record.player_kind,
                        reason,
                    }),
                ));
            }
        }

        // 音乐客户端刚启动但尚未播放时通常还不是 Windows CurrentSession，也没有
        // 活动序号。只在没有播放/暂停首选项时使用已完成初始化的四家播放器会话，
        // 让 Bar 能展示客户端保留的最后媒体；后续真实播放仍会覆盖这个回退选择。
        let detected_preferred = observations.iter().rev().find_map(|observation| {
            let record = state.records.get(&observation.session_key)?;
            if record.player_kind == MediaPlayerKind::Other || record.title.is_none() {
                return None;
            }
            Some((observation.session.clone(), observation.session_key, record))
        });
        if let Some((session, session_key, record)) = detected_preferred {
            return Ok((
                Some(session),
                Some(SelectedMediaSession {
                    session_key,
                    source_app_id: record.source_app_id.clone(),
                    player_kind: record.player_kind,
                    reason: MediaSelectionReason::DetectedPreferred,
                }),
            ));
        }

        // SessionsChanged 与 CurrentSessionChanged 并不保证到达顺序。播放器刚退出时，
        // Windows CurrentSession 可能短暂仍指向已移除对象，因此只接受最新会话列表
        // 中仍有对应观察项的回退会话，避免 Bar 长期保留退出前的最后一首歌曲。
        let fallback = windows_current_session.and_then(|session| {
            let session_key = session_key(&session);
            if !observations
                .iter()
                .any(|observation| observation.session_key == session_key)
            {
                return None;
            }
            let source_app_id = session.SourceAppUserModelId().ok()?.to_string();
            Some((
                session,
                SelectedMediaSession {
                    session_key,
                    player_kind: identify_media_player(&source_app_id),
                    source_app_id,
                    reason: MediaSelectionReason::WindowsCurrentFallback,
                },
            ))
        });
        Ok(match fallback {
            Some((session, selection)) => (Some(session), Some(selection)),
            None => (None, None),
        })
    }
}

/// 订阅一个会话的播放与媒体属性变化，并提交首次状态读取。
fn subscribe_session_activity(
    session_key: u64,
    session: &GlobalSystemMediaTransportControlsSession,
    sender: &SyncSender<ActivityMessage>,
) -> Result<SessionActivityObservation, String> {
    let playback_sender = sender.clone();
    let playback_handler: TypedEventHandler<
        GlobalSystemMediaTransportControlsSession,
        PlaybackInfoChangedEventArgs,
    > = TypedEventHandler::new(
        move |sender: windows::core::Ref<'_, GlobalSystemMediaTransportControlsSession>,
              _: windows::core::Ref<'_, PlaybackInfoChangedEventArgs>| {
            if let Some(session) = sender.as_ref() {
                send_activity_request(
                    &playback_sender,
                    ActivityRequest {
                        session_key,
                        session: session.clone(),
                        kind: ActivityRequestKind::PlaybackChanged,
                    },
                );
            }
            Ok(())
        },
    );
    let playback_token = session
        .PlaybackInfoChanged(&playback_handler)
        .map_err(|error| format!("无法订阅媒体活动播放变化：{error}"))?;

    let properties_sender = sender.clone();
    let properties_handler: TypedEventHandler<
        GlobalSystemMediaTransportControlsSession,
        MediaPropertiesChangedEventArgs,
    > = TypedEventHandler::new(
        move |sender: windows::core::Ref<'_, GlobalSystemMediaTransportControlsSession>,
              _: windows::core::Ref<'_, MediaPropertiesChangedEventArgs>| {
            if let Some(session) = sender.as_ref() {
                send_activity_request(
                    &properties_sender,
                    ActivityRequest {
                        session_key,
                        session: session.clone(),
                        kind: ActivityRequestKind::MediaPropertiesChanged,
                    },
                );
            }
            Ok(())
        },
    );
    let media_properties_token = match session.MediaPropertiesChanged(&properties_handler) {
        Ok(token) => token,
        Err(error) => {
            let _ = session.RemovePlaybackInfoChanged(playback_token);
            return Err(format!("无法订阅媒体活动属性变化：{error}"));
        }
    };

    send_activity_request(
        sender,
        ActivityRequest {
            session_key,
            session: session.clone(),
            kind: ActivityRequestKind::Initialize,
        },
    );

    Ok(SessionActivityObservation {
        session_key,
        session: session.clone(),
        playback_token,
        media_properties_token,
    })
}

/// 提交活动任务；后台线程已经退出时只记录一次警告。
fn send_activity_request(sender: &SyncSender<ActivityMessage>, request: ActivityRequest) {
    match sender.try_send(ActivityMessage::Update(request)) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            log::debug!("媒体活动队列已满，本次变化将由后续状态读取覆盖")
        }
        Err(TrySendError::Disconnected(_)) => log::warn!("无法提交媒体活动更新：后台线程已停止"),
    }
}

/// 在固定 MTA apartment 中串行判断播放开始、切歌和当前会话变化。
fn run_activity_worker<R: Runtime>(
    receiver: Receiver<ActivityMessage>,
    state: Arc<Mutex<ActivityState>>,
    app: AppHandle<R>,
) {
    if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
        log::error!("无法初始化媒体活动线程的 WinRT 环境：{error}");
        return;
    }

    while let Ok(message) = receiver.recv() {
        let ActivityMessage::Update(request) = message else {
            break;
        };
        if let Err(error) = apply_activity_request(&state, request) {
            log::warn!("无法更新媒体会话活动：{error}");
            continue;
        }
        emit_activity_state(&state, &app);
    }

    // SAFETY: 当前线程成功初始化过 WinRT apartment，并且即将退出。
    unsafe { RoUninitialize() };
}

/// 根据任务类型读取必要的最少数据，并更新对应会话记录。
fn apply_activity_request(
    state: &Arc<Mutex<ActivityState>>,
    request: ActivityRequest,
) -> Result<(), String> {
    match request.kind {
        ActivityRequestKind::Initialize => {
            let playback_state = read_playback_state(&request.session)?;
            let is_playing = playback_state.is_playing;
            let (title, artist) = read_track_identity(&request.session)?;
            let mut state = state
                .lock()
                .map_err(|_| "媒体活动记录锁已损坏".to_owned())?;
            let Some(record) = state.records.get_mut(&request.session_key) else {
                return Ok(());
            };
            let was_initialized = record.title.is_some();
            let started_playing = was_initialized && is_playing && !record.is_playing;
            let track_changed = was_initialized
                && (record.title.as_deref() != Some(title.as_str())
                    || record.artist.as_deref() != Some(artist.as_str()));
            record.title = Some(title);
            record.artist = Some(artist);
            record.is_playing = is_playing;
            record.is_paused = playback_state.is_paused;
            let activity_reason = if !was_initialized && is_playing {
                Some(MediaActivityReason::DetectedPlaying)
            } else if track_changed {
                Some(MediaActivityReason::TrackChanged)
            } else if started_playing {
                Some(MediaActivityReason::PlaybackStarted)
            } else {
                None
            };
            if let Some(reason) = activity_reason {
                mark_activity(&mut state, request.session_key, reason);
            }
        }
        ActivityRequestKind::PlaybackChanged => {
            let playback_state = read_playback_state(&request.session)?;
            let is_playing = playback_state.is_playing;
            let mut state = state
                .lock()
                .map_err(|_| "媒体活动记录锁已损坏".to_owned())?;
            let started_playing = state
                .records
                .get(&request.session_key)
                .is_some_and(|record| is_playing && !record.is_playing);
            if let Some(record) = state.records.get_mut(&request.session_key) {
                record.is_playing = is_playing;
                record.is_paused = playback_state.is_paused;
            }
            if started_playing {
                mark_activity(
                    &mut state,
                    request.session_key,
                    MediaActivityReason::PlaybackStarted,
                );
            }
        }
        ActivityRequestKind::MediaPropertiesChanged => {
            let (title, artist) = read_track_identity(&request.session)?;
            let mut state = state
                .lock()
                .map_err(|_| "媒体活动记录锁已损坏".to_owned())?;
            let track_changed = state
                .records
                .get(&request.session_key)
                .is_some_and(|record| {
                    record
                        .title
                        .as_deref()
                        .is_some_and(|old_title| old_title != title.as_str())
                        || record
                            .artist
                            .as_deref()
                            .is_some_and(|old_artist| old_artist != artist.as_str())
                });
            if let Some(record) = state.records.get_mut(&request.session_key) {
                record.title = Some(title);
                record.artist = Some(artist);
            }
            if track_changed {
                mark_activity(
                    &mut state,
                    request.session_key,
                    MediaActivityReason::TrackChanged,
                );
            }
        }
        ActivityRequestKind::BecameCurrent => {
            let mut state = state
                .lock()
                .map_err(|_| "媒体活动记录锁已损坏".to_owned())?;
            mark_activity(
                &mut state,
                request.session_key,
                MediaActivityReason::BecameCurrent,
            );
        }
    }

    Ok(())
}

/// 将指定会话的时间、顺序和原因更新为最新活动。
fn mark_activity(state: &mut ActivityState, session_key: u64, reason: MediaActivityReason) {
    state.next_activity_sequence = state.next_activity_sequence.saturating_add(1);
    let sequence = state.next_activity_sequence;
    let timestamp = current_unix_time_ms();
    if let Some(record) = state.records.get_mut(&session_key) {
        record.last_activity_at_unix_ms = Some(timestamp);
        record.activity_sequence = Some(sequence);
        record.last_activity_reason = Some(reason);
    }
}

/// 选择器需要区分正在播放、暂停和其他不可保留状态。
struct ActivityPlaybackState {
    is_playing: bool,
    is_paused: bool,
}

/// 读取当前会话是否处于 Playing 或 Paused。
fn read_playback_state(
    session: &GlobalSystemMediaTransportControlsSession,
) -> Result<ActivityPlaybackState, String> {
    let status = session
        .GetPlaybackInfo()
        .and_then(|playback_info| playback_info.PlaybackStatus())
        .map_err(|error| format!("无法读取媒体活动播放状态：{error}"))?;
    Ok(ActivityPlaybackState {
        is_playing: status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing,
        is_paused: status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused,
    })
}

/// 只读取用于判断切歌的标题和歌手，不读取或传输封面。
fn read_track_identity(
    session: &GlobalSystemMediaTransportControlsSession,
) -> Result<(String, String), String> {
    let properties = session
        .TryGetMediaPropertiesAsync()
        .and_then(|operation| operation.get())
        .map_err(|error| format!("无法读取媒体活动曲目信息：{error}"))?;
    let title = bounded_media_text(
        properties
            .Title()
            .map_err(|error| format!("无法读取媒体活动标题：{error}"))?
            .to_string(),
    );
    let artist = bounded_media_text(
        properties
            .Artist()
            .map_err(|error| format!("无法读取媒体活动歌手：{error}"))?
            .to_string(),
    );
    Ok((title, artist))
}

/// 使用 WinRT 对象地址生成仅在本次进程内有效的稳定会话标识。
pub(crate) fn session_key(session: &GlobalSystemMediaTransportControlsSession) -> u64 {
    // COM 要求同一对象查询得到的 IUnknown 地址保持一致，因此它比具体接口指针更适合作为身份。
    session
        .cast::<IUnknown>()
        .map(|identity| identity.as_raw() as usize as u64)
        .unwrap_or_else(|_| session.as_raw() as usize as u64)
}

/// 返回当前 Unix 毫秒时间；系统时钟异常时使用零值而不是中断活动跟踪。
fn current_unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

/// 将内部活动记录转换为按会话标识稳定排序的诊断数组。
fn activity_snapshots(state: &ActivityState) -> Vec<MediaSessionActivity> {
    let mut snapshots = state
        .records
        .iter()
        .map(|(session_key, record)| MediaSessionActivity {
            session_key: *session_key,
            source_app_id: record.source_app_id.clone(),
            player_kind: record.player_kind,
            title: record.title.clone(),
            artist: record.artist.clone(),
            is_playing: record.is_playing,
            is_paused: record.is_paused,
            last_activity_at_unix_ms: record.last_activity_at_unix_ms,
            activity_sequence: record.activity_sequence,
            last_activity_reason: record.last_activity_reason,
        })
        .collect::<Vec<_>>();
    snapshots.sort_by_key(|snapshot| snapshot.session_key);
    snapshots
}

/// 纯粹依据活动记录选择四家播放器，不读取 WinRT 或修改任何状态。
fn select_preferred_session_key(state: &ActivityState) -> Option<u64> {
    super::selection::select_preferred_session_key(state.records.iter().map(
        |(session_key, record)| MediaSelectionCandidate {
            session_key: *session_key,
            player_kind: record.player_kind,
            is_playing: record.is_playing,
            is_paused: record.is_paused,
            activity_sequence: record.activity_sequence,
        },
    ))
}

/// 广播当前全部会话活动，供设置页验证活动序号不会被时间轴刷新改变。
fn emit_activity_state<R: Runtime>(state: &Arc<Mutex<ActivityState>>, app: &AppHandle<R>) {
    let snapshots = match state.lock() {
        Ok(state) => activity_snapshots(&state),
        Err(_) => {
            log::warn!("无法广播媒体活动：记录锁已损坏");
            return;
        }
    };

    if let Err(error) = app.emit(MEDIA_SESSION_ACTIVITIES_CHANGED_EVENT, snapshots) {
        log::warn!("无法广播媒体会话活动变化：{error}");
    }
}
