use tauri::{AppHandle, State};

use crate::{settings::AppSettings, state::AppState};

/// 返回应用进程当前持有的完整设置。
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    state.settings()
}

/// 将完整设置交给应用服务执行同步、持久化和广播事务。
#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    crate::settings_update::apply(&app, &state, settings)
}
