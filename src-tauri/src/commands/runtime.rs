use std::time::UNIX_EPOCH;

use serde::Serialize;
use tauri::State;

use crate::state::AppState;

/// 前端可安全读取的应用运行信息。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    application_version: String,
    started_at_unix_ms: u64,
}

/// 从全局 `AppState` 读取版本和启动时间，并转换为前端可序列化的数据。
#[tauri::command]
pub fn get_runtime_info(state: State<'_, AppState>) -> Result<RuntimeInfo, String> {
    let started_at = state
        .started_at()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("无法转换应用启动时间：{error}"))?;

    let started_at_unix_ms = u64::try_from(started_at.as_millis())
        .map_err(|_| "应用启动时间超出可表示范围".to_owned())?;

    Ok(RuntimeInfo {
        application_version: state.application_version().to_owned(),
        started_at_unix_ms,
    })
}
