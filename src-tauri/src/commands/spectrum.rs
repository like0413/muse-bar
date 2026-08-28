use tauri::{AppHandle, Manager, State};

use crate::{
    media::SystemMediaManager, spectrum::SpectrumManager, volume::ApplicationVolumeManager,
};

/// 为调用方仍指向的当前媒体会话启动按进程音频频谱采集。
#[tauri::command]
pub async fn start_application_spectrum(
    app: AppHandle,
    media: State<'_, SystemMediaManager>,
    volume: State<'_, ApplicationVolumeManager>,
    expected_session_key: u64,
    frame_rate: u8,
) -> Result<(), String> {
    if frame_rate != 20 && frame_rate != 30 {
        return Err("频谱刷新率只支持 20 或 30 FPS".to_owned());
    }
    let identity = media.current_volume_identity(&app, expected_session_key)?;
    let process_id = volume
        .resolve_capture_process(identity.clone())
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "没有找到当前媒体应用的活动音频进程".to_owned())?;
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // 进程枚举可能耗时；真正启动前再次校验，避免延迟请求覆盖新播放器。
        worker_app
            .state::<SystemMediaManager>()
            .current_volume_identity(&worker_app, expected_session_key)?;
        worker_app.state::<SpectrumManager>().start(
            worker_app.clone(),
            expected_session_key,
            process_id,
            identity,
            frame_rate,
        )
    })
    .await
    .map_err(|error| format!("频谱启动任务意外停止：{error}"))?
}

/// 停止当前频谱采集；重复停止不会报错。
#[tauri::command]
pub async fn stop_application_spectrum(
    app: AppHandle,
    expected_session_key: u64,
) -> Result<(), String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        worker_app
            .state::<SpectrumManager>()
            .stop_session(expected_session_key);
    })
    .await
    .map_err(|error| format!("频谱停止任务意外停止：{error}"))
}
