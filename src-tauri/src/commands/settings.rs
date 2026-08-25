use tauri::{AppHandle, Emitter, State};

use crate::{settings::AppSettings, state::AppState};

const SETTINGS_CHANGED_EVENT: &str = "settings-changed";

/// 返回应用进程当前持有的完整设置。
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    state.settings()
}

/// 同步系统启动项、持久化完整设置，并向所有窗口广播最新结果。
#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    let previous_settings = state.settings()?;
    let target_monitor_changed = previous_settings.target_monitor != settings.target_monitor;
    let launch_on_startup_after_update = settings.launch_on_startup;
    if previous_settings.launch_on_startup != launch_on_startup_after_update {
        crate::autostart::synchronize(&app, launch_on_startup_after_update)?;
    }

    let updated_settings = match state.update_settings(&app, settings) {
        Ok(settings) => settings,
        Err(save_error) => {
            if previous_settings.launch_on_startup != launch_on_startup_after_update {
                let rollback_result =
                    crate::autostart::synchronize(&app, previous_settings.launch_on_startup);
                if let Err(rollback_error) = rollback_result {
                    return Err(format!(
                        "{save_error}；同时无法恢复原开机启动状态：{rollback_error}"
                    ));
                }
            }
            return Err(save_error);
        }
    };

    app.emit(SETTINGS_CHANGED_EVENT, &updated_settings)
        .map_err(|error| format!("设置已保存，但无法广播设置变化：{error}"))?;

    if target_monitor_changed {
        // 只有跨显示器时才需要更换原生 Child 的父任务栏。同一任务栏中的位置和
        // 偏移由 Bar 收到设置事件后直接调整，避免恢复流程与宽度上报并发移动窗口。
        if let Err(error) = crate::explorer_monitor::request_recovery() {
            log::warn!("设置已保存，但无法立即更新 Bar 任务栏位置：{error}");
        }
    }

    Ok(updated_settings)
}
