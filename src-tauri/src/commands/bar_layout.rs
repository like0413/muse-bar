use tauri::{AppHandle, State};

use crate::state::{AppState, BarWidthMeasurement};

/// 接收前端自然宽度，并交给应用服务调整原生 Bar。
#[tauri::command]
pub fn report_bar_content_width(
    app: AppHandle,
    state: State<'_, AppState>,
    natural_width: f64,
) -> Result<BarWidthMeasurement, String> {
    crate::bar_layout::apply_content_width(&app, &state, natural_width)
}
