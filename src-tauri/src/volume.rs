use std::{
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
        Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use windows::{
    core::{Interface, GUID, PWSTR},
    Win32::{
        Foundation::{
            CloseHandle, APPMODEL_ERROR_NO_APPLICATION, ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS,
            HANDLE,
        },
        Media::Audio::{
            eRender, AudioSessionStateActive, IAudioSessionControl2, IAudioSessionManager2,
            IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
        },
        Storage::Packaging::Appx::GetApplicationUserModelId,
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, CLSCTX_INPROC_SERVER,
                COINIT_MULTITHREADED,
            },
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    },
};

use crate::{
    background_worker::{join_with_timeout, WORKER_SHUTDOWN_TIMEOUT},
    media::MediaVolumeIdentity,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const PROCESS_PATH_BUFFER_LENGTH: usize = 32_768;
const EVENT_CONTEXT: GUID = GUID::from_u128(0x8cb0c0fe_536c_4ec3_b446_17c1a1944981);

/// 前端允许执行的应用音量动作。
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ApplicationVolumeAction {
    SetLevel {
        #[serde(rename = "levelPercent")]
        level_percent: u8,
    },
    Adjust {
        #[serde(rename = "deltaPercent")]
        delta_percent: i8,
    },
    ToggleMute,
}

/// 当前媒体应用对应的 Core Audio 音量状态。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApplicationVolumeState {
    session_key: u64,
    level_percent: u8,
    muted: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum ApplicationVolumeErrorCode {
    NoMediaSession,
    SessionChanged,
    AudioSessionUnavailable,
    WindowsApi,
    WorkerUnavailable,
}

/// 应用音量 IPC 使用的稳定错误类别与可读说明。
#[derive(Debug, Error, Serialize)]
#[error("{message}")]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApplicationVolumeError {
    code: ApplicationVolumeErrorCode,
    message: String,
}

impl ApplicationVolumeError {
    fn new(code: ApplicationVolumeErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn media_identity(message: String) -> Self {
        let code = if message.contains("已经切换") {
            ApplicationVolumeErrorCode::SessionChanged
        } else {
            ApplicationVolumeErrorCode::NoMediaSession
        };
        Self::new(code, message)
    }

    fn windows_api(message: impl Into<String>) -> Self {
        Self::new(ApplicationVolumeErrorCode::WindowsApi, message)
    }
}

enum VolumeRequest {
    Query {
        identity: MediaVolumeIdentity,
        response: SyncSender<Result<Option<ApplicationVolumeState>, ApplicationVolumeError>>,
    },
    Control {
        identity: MediaVolumeIdentity,
        action: ApplicationVolumeAction,
        response: SyncSender<Result<ApplicationVolumeState, ApplicationVolumeError>>,
    },
    Shutdown,
}

/// 串行执行 Core Audio COM 调用，避免音频接口跨 apartment 使用。
pub(crate) struct ApplicationVolumeManager {
    requests: SyncSender<VolumeRequest>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl ApplicationVolumeManager {
    pub(crate) fn start() -> Result<Self, String> {
        let (requests, receiver) = mpsc::sync_channel(32);
        let worker = thread::Builder::new()
            .name("muse-bar-application-volume".to_owned())
            .spawn(move || run_volume_worker(receiver))
            .map_err(|error| format!("无法启动应用音量线程：{error}"))?;
        Ok(Self {
            requests,
            worker: Mutex::new(Some(worker)),
        })
    }

    pub(crate) fn query(
        &self,
        identity: MediaVolumeIdentity,
    ) -> Result<Option<ApplicationVolumeState>, ApplicationVolumeError> {
        self.request(|response| VolumeRequest::Query { identity, response })
    }

    pub(crate) fn control(
        &self,
        identity: MediaVolumeIdentity,
        action: ApplicationVolumeAction,
    ) -> Result<ApplicationVolumeState, ApplicationVolumeError> {
        self.request(|response| VolumeRequest::Control {
            identity,
            action,
            response,
        })
    }

    fn request<T>(
        &self,
        create_request: impl FnOnce(SyncSender<Result<T, ApplicationVolumeError>>) -> VolumeRequest,
    ) -> Result<T, ApplicationVolumeError> {
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        self.requests
            .send(create_request(response_sender))
            .map_err(|_| {
                ApplicationVolumeError::new(
                    ApplicationVolumeErrorCode::WorkerUnavailable,
                    "应用音量线程已经停止",
                )
            })?;
        response_receiver
            .recv_timeout(REQUEST_TIMEOUT)
            .map_err(|error| {
                let message = match error {
                    RecvTimeoutError::Timeout => "等待应用音量响应超时",
                    RecvTimeoutError::Disconnected => "应用音量线程意外停止",
                };
                ApplicationVolumeError::new(ApplicationVolumeErrorCode::WorkerUnavailable, message)
            })?
    }

    pub(crate) fn request_shutdown(&self) {
        let _ = self.requests.try_send(VolumeRequest::Shutdown);
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(worker) = worker.take() {
                join_with_timeout(worker, "应用音量", WORKER_SHUTDOWN_TIMEOUT);
            }
        }
    }
}

impl Drop for ApplicationVolumeManager {
    fn drop(&mut self) {
        let _ = self.requests.try_send(VolumeRequest::Shutdown);
    }
}

fn run_volume_worker(receiver: Receiver<VolumeRequest>) {
    if let Err(error) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        log::error!("无法初始化应用音量 COM 线程：{error}");
        return;
    }
    let _com_guard = ComGuard;

    while let Ok(request) = receiver.recv() {
        match request {
            VolumeRequest::Query { identity, response } => {
                let _ = response.send(query_application_volume(&identity));
            }
            VolumeRequest::Control {
                identity,
                action,
                response,
            } => {
                let _ = response.send(control_application_volume(&identity, action));
            }
            VolumeRequest::Shutdown => break,
        }
    }
}

struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if let Err(error) = unsafe { CloseHandle(self.0) } {
            log::warn!("无法关闭音频会话进程句柄：{error}");
        }
    }
}

struct MatchedAudioSession {
    volume: ISimpleAudioVolume,
    level: f32,
    muted: bool,
    active: bool,
    match_score: u8,
}

fn query_application_volume(
    identity: &MediaVolumeIdentity,
) -> Result<Option<ApplicationVolumeState>, ApplicationVolumeError> {
    let sessions = collect_matching_sessions(identity)?;
    Ok(
        summarize_sessions(&sessions).map(|(level, muted)| ApplicationVolumeState {
            session_key: identity.session_key,
            level_percent: scalar_to_percent(level),
            muted,
        }),
    )
}

fn control_application_volume(
    identity: &MediaVolumeIdentity,
    action: ApplicationVolumeAction,
) -> Result<ApplicationVolumeState, ApplicationVolumeError> {
    let sessions = collect_matching_sessions(identity)?;
    let (current_level, currently_muted) = summarize_sessions(&sessions).ok_or_else(|| {
        ApplicationVolumeError::new(
            ApplicationVolumeErrorCode::AudioSessionUnavailable,
            "没有找到当前媒体应用的音频会话",
        )
    })?;
    let current_percent = i16::from(scalar_to_percent(current_level));
    let (target_percent, target_muted) = match action {
        ApplicationVolumeAction::SetLevel { level_percent } => {
            let level = level_percent.min(100);
            (Some(level), (level > 0).then_some(false))
        }
        ApplicationVolumeAction::Adjust { delta_percent } => {
            let level = (current_percent + i16::from(delta_percent)).clamp(0, 100) as u8;
            (Some(level), (level > 0).then_some(false))
        }
        ApplicationVolumeAction::ToggleMute => (None, Some(!currently_muted)),
    };

    for session in &sessions {
        if let Some(level) = target_percent {
            unsafe {
                session
                    .volume
                    .SetMasterVolume(f32::from(level) / 100.0, &EVENT_CONTEXT)
                    .map_err(|error| {
                        ApplicationVolumeError::windows_api(format!(
                            "无法设置当前应用音量：{error}"
                        ))
                    })?;
            }
        }
        if let Some(muted) = target_muted {
            unsafe {
                session
                    .volume
                    .SetMute(muted, &EVENT_CONTEXT)
                    .map_err(|error| {
                        ApplicationVolumeError::windows_api(format!(
                            "无法设置当前应用静音状态：{error}"
                        ))
                    })?;
            }
        }
    }

    Ok(ApplicationVolumeState {
        session_key: identity.session_key,
        level_percent: target_percent.unwrap_or_else(|| scalar_to_percent(current_level)),
        muted: target_muted.unwrap_or(currently_muted),
    })
}

/// 将同一播放器的多个 Core Audio 会话汇总为用户实际听到的应用级状态。
/// 任一匹配会话未静音即表示应用未静音，音量取最高的活动会话，避免后台零音量
/// 子进程把仍在发声的播放器显示成静音。
fn summarize_sessions(sessions: &[MatchedAudioSession]) -> Option<(f32, bool)> {
    let highest_match_score = sessions.iter().map(|session| session.match_score).max()?;
    let strongest_matches = sessions
        .iter()
        .filter(|session| session.match_score == highest_match_score);
    let has_active_session = strongest_matches.clone().any(|session| session.active);
    let relevant_sessions =
        strongest_matches.filter(|session| !has_active_session || session.active);
    let (level, muted) = relevant_sessions.fold((0.0_f32, true), |(level, muted), session| {
        (level.max(session.level), muted && session.muted)
    });
    Some((level, muted))
}

fn scalar_to_percent(level: f32) -> u8 {
    (level.clamp(0.0, 1.0) * 100.0).round() as u8
}

fn collect_matching_sessions(
    identity: &MediaVolumeIdentity,
) -> Result<Vec<MatchedAudioSession>, ApplicationVolumeError> {
    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER).map_err(|error| {
            ApplicationVolumeError::windows_api(format!("无法创建音频设备枚举器：{error}"))
        })?
    };
    let devices = unsafe {
        enumerator
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .map_err(|error| {
                ApplicationVolumeError::windows_api(format!("无法枚举音频输出设备：{error}"))
            })?
    };
    let device_count = unsafe { devices.GetCount() }.map_err(|error| {
        ApplicationVolumeError::windows_api(format!("无法读取音频输出设备数量：{error}"))
    })?;
    let mut matches = Vec::new();

    for device_index in 0..device_count {
        let device = unsafe { devices.Item(device_index) }.map_err(|error| {
            ApplicationVolumeError::windows_api(format!("无法读取音频输出设备：{error}"))
        })?;
        let manager: IAudioSessionManager2 =
            unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(|error| {
                ApplicationVolumeError::windows_api(format!("无法打开音频会话管理器：{error}"))
            })?;
        let sessions = unsafe { manager.GetSessionEnumerator() }.map_err(|error| {
            ApplicationVolumeError::windows_api(format!("无法枚举应用音频会话：{error}"))
        })?;
        let session_count = unsafe { sessions.GetCount() }.map_err(|error| {
            ApplicationVolumeError::windows_api(format!("无法读取应用音频会话数量：{error}"))
        })?;

        for session_index in 0..session_count {
            let control = match unsafe { sessions.GetSession(session_index) } {
                Ok(control) => control,
                Err(error) => {
                    log::debug!("跳过无法读取的音频会话：{error}");
                    continue;
                }
            };
            let control2: IAudioSessionControl2 = match control.cast() {
                Ok(control) => control,
                Err(error) => {
                    log::debug!("跳过缺少 IAudioSessionControl2 的会话：{error}");
                    continue;
                }
            };
            let process_id = match unsafe { control2.GetProcessId() } {
                Ok(process_id) if process_id != 0 => process_id,
                _ => continue,
            };
            let process = match ProcessIdentity::read(process_id) {
                Ok(process) => process,
                Err(error) => {
                    log::debug!("跳过无法识别进程的音频会话：{error}");
                    continue;
                }
            };
            let Some(match_score) = process.match_score(identity) else {
                continue;
            };
            let volume: ISimpleAudioVolume = match control.cast() {
                Ok(volume) => volume,
                Err(error) => {
                    log::debug!("跳过缺少 ISimpleAudioVolume 的会话：{error}");
                    continue;
                }
            };
            let level = match unsafe { volume.GetMasterVolume() } {
                Ok(level) => level,
                Err(error) => {
                    log::debug!("跳过无法读取音量的会话：{error}");
                    continue;
                }
            };
            let muted = unsafe { volume.GetMute() }
                .map(|muted| muted.as_bool())
                .unwrap_or(false);
            let active = unsafe { control.GetState() }
                .map(|state| state == AudioSessionStateActive)
                .unwrap_or(false);
            matches.push(MatchedAudioSession {
                volume,
                level,
                muted,
                active,
                match_score,
            });
        }
    }

    Ok(matches)
}

struct ProcessIdentity {
    image_path: PathBuf,
    app_user_model_id: Option<String>,
}

impl ProcessIdentity {
    fn read(process_id: u32) -> Result<Self, String> {
        let handle = unsafe {
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id)
                .map(OwnedHandle)
                .map_err(|error| format!("无法打开进程 {process_id}：{error}"))?
        };
        let image_path = query_process_image_path(handle.0)?;
        // AUMID 是最高优先级匹配信息，但传统桌面进程可能拒绝或暂时无法提供它。
        // 这种失败不应丢弃仍可由可执行文件名准确识别的音频会话。
        let app_user_model_id = query_process_app_user_model_id(handle.0)
            .inspect_err(|error| log::debug!("无法读取音频进程 AUMID，将回退到进程名：{error}"))
            .unwrap_or_default();
        Ok(Self {
            image_path,
            app_user_model_id,
        })
    }

    fn match_score(&self, identity: &MediaVolumeIdentity) -> Option<u8> {
        if self
            .app_user_model_id
            .as_ref()
            .is_some_and(|app_id| app_id.eq_ignore_ascii_case(identity.source_app_id.trim()))
        {
            return Some(3);
        }
        let image_name = self
            .image_path
            .file_name()
            .and_then(|name| name.to_str())?
            .to_ascii_lowercase();
        let image_stem = self
            .image_path
            .file_stem()
            .and_then(|name| name.to_str())?
            .to_ascii_lowercase();
        let normalized_source = identity.source_app_id.to_ascii_lowercase();
        if normalized_source.contains(&image_name) || normalized_source.contains(&image_stem) {
            return Some(2);
        }
        identity
            .player_kind
            .matches_process_image(&image_name)
            .then_some(1)
    }
}

fn query_process_image_path(process: HANDLE) -> Result<PathBuf, String> {
    let mut buffer = vec![0_u16; PROCESS_PATH_BUFFER_LENGTH];
    let mut length = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
        .map_err(|error| format!("无法读取进程路径：{error}"))?;
    }
    buffer.truncate(length as usize);
    Ok(PathBuf::from(OsString::from_wide(&buffer)))
}

fn query_process_app_user_model_id(process: HANDLE) -> Result<Option<String>, String> {
    let mut length = 0_u32;
    let first_result = unsafe { GetApplicationUserModelId(process, &mut length, None) };
    if first_result == APPMODEL_ERROR_NO_APPLICATION {
        return Ok(None);
    }
    if first_result != ERROR_INSUFFICIENT_BUFFER {
        return Err(format!("无法读取进程 AUMID 长度：{}", first_result.0));
    }

    let mut buffer = vec![0_u16; length as usize];
    let result = unsafe {
        GetApplicationUserModelId(process, &mut length, Some(PWSTR(buffer.as_mut_ptr())))
    };
    if result != ERROR_SUCCESS {
        return Err(format!("无法读取进程 AUMID：{}", result.0));
    }
    let used_length = length.saturating_sub(1) as usize;
    buffer.truncate(used_length);
    Ok(Some(String::from_utf16_lossy(&buffer)))
}
