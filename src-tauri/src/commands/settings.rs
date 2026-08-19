use tauri::{AppHandle, Emitter, State};

use crate::{settings::AppSettings, state::AppState};

const SETTINGS_CHANGED_EVENT: &str = "settings-changed";

/// 返回应用进程当前持有的完整设置。
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    state.settings()
}

/// 持久化完整设置、更新共享状态，并向所有窗口广播最新结果。
#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    let updated_settings = state.update_settings(&app, settings)?;

    app.emit(SETTINGS_CHANGED_EVENT, &updated_settings)
        .map_err(|error| format!("设置已保存，但无法广播设置变化：{error}"))?;

    Ok(updated_settings)
}
