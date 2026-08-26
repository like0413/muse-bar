use tauri::{AppHandle, State};

use crate::updater::{self, UpdateManager, UpdateStatus};

/// 返回后台更新器当前持有的完整状态快照。
#[tauri::command]
pub fn get_update_status(state: State<'_, UpdateManager>) -> Result<UpdateStatus, String> {
    state.snapshot()
}

/// 由设置页显式触发一次更新检查，并向用户暴露检查失败原因。
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<UpdateStatus, String> {
    updater::check_for_update(&app, false).await
}

/// 重新确认远端版本后，下载并安装用户明确选择的版本。
#[tauri::command]
pub async fn install_update(app: AppHandle, expected_version: String) -> Result<(), String> {
    updater::install_update(&app, &expected_version).await
}
