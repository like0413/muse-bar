use tauri::{AppHandle, State};

use crate::state::{AppState, BarWidthMeasurement};

/// 接收前端自然宽度，并交给应用服务调整原生 Bar。
#[tauri::command]
pub fn report_bar_content_width(
    app: AppHandle,
    state: State<'_, AppState>,
    natural_width: f64,
    additional_width: f64,
    reduce_motion: bool,
) -> Result<BarWidthMeasurement, String> {
    crate::taskbar::apply_content_width(
        &app,
        &state,
        natural_width,
        additional_width,
        reduce_motion,
    )
}
