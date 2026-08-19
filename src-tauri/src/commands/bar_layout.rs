use tauri::{AppHandle, Manager, State};

use crate::{
    child_host,
    state::{AppState, BarWidthMeasurement},
    taskbar,
};

/// 接收 Bar 前端测得的自然逻辑宽度，并返回应用设置限制后的目标宽度。
#[tauri::command]
pub fn report_bar_content_width(
    app: AppHandle,
    state: State<'_, AppState>,
    natural_width: f64,
) -> Result<BarWidthMeasurement, String> {
    let measurement = state.report_bar_content_width(natural_width)?;
    let bar_window = app
        .get_window("bar")
        .ok_or_else(|| "无法调整宽度：Bar 原生宿主不存在".to_owned())?;
    let taskbar = taskbar::find_main_taskbar()?;
    let taskbar_dpi = taskbar::read_taskbar_dpi(&taskbar)?;
    let target_physical_width =
        (f64::from(measurement.target_width()) * taskbar_dpi.scale_factor()).round();
    let target_physical_width = i32::try_from(target_physical_width as i64)
        .map_err(|_| "Bar 目标物理宽度超出可表示范围".to_owned())?;
    let (animation_revision, latest_animation_revision) = state.begin_bar_width_animation();

    child_host::animate_window_width(
        bar_window,
        target_physical_width,
        animation_revision,
        latest_animation_revision,
    )?;

    Ok(measurement)
}
