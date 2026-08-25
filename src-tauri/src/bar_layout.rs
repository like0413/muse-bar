use tauri::{AppHandle, Manager};

use crate::{
    child_host,
    state::{AppState, BarWidthMeasurement},
    taskbar,
};

/// 按内容策略计算并应用原生 Bar 宽度。
pub(crate) fn apply_content_width(
    app: &AppHandle,
    state: &AppState,
    natural_width: f64,
) -> Result<BarWidthMeasurement, String> {
    let bar_window = app
        .get_window("bar")
        .ok_or_else(|| "无法调整宽度：Bar 原生宿主不存在".to_owned())?;
    let bar_webview = app
        .get_webview("bar")
        .ok_or_else(|| "无法调整宽度：Bar WebView 不存在".to_owned())?;
    let settings = state.settings()?;
    let taskbar = taskbar::find_taskbar(&settings.target_monitor)?;
    let taskbar_rect = taskbar::read_taskbar_rect(&taskbar)?;
    let taskbar_dpi = taskbar::read_taskbar_dpi(&taskbar)?;
    let available_span = if settings.lyrics_enabled {
        // 只读取 Explorer 自己的 XAML 宿主。它与 Muse Bar 是兄弟窗口，
        // 因此同步命令不会反向进入正在等待返回的 Bar WebView。
        let occupied_regions =
            crate::taskbar_occupancy::read_positioning_regions(&taskbar, &taskbar_rect);
        Some(crate::taskbar_occupancy::resolve_available_span(
            settings.position,
            &taskbar_rect,
            &occupied_regions,
        ))
    } else {
        None
    };
    let available_logical_width = available_span.map(|span| {
        taskbar_dpi
            .physical_to_logical(span.width())
            .round()
            .max(1.0) as u32
    });
    let measurement = state.report_bar_content_width(natural_width, available_logical_width)?;
    if !child_host::is_attached_to_taskbar(&bar_window, &taskbar) {
        // 目标显示器刚改变时，先交给恢复线程完成重新挂载，避免在旧任务栏坐标系中移动窗口。
        crate::explorer_monitor::request_recovery()?;
        return Ok(measurement.deferred());
    }
    let target_physical_width = match available_span {
        Some(span) => span.width(),
        None => {
            let width =
                (f64::from(measurement.target_width()) * taskbar_dpi.scale_factor()).round();
            i32::try_from(width as i64).map_err(|_| "Bar 目标物理宽度超出可表示范围".to_owned())?
        }
    };
    let (animation_revision, latest_animation_revision) = state.begin_bar_width_animation();

    child_host::animate_window_width(child_host::WindowWidthAnimationRequest {
        bar_window,
        bar_webview,
        taskbar: &taskbar,
        taskbar_rect: &taskbar_rect,
        taskbar_dpi: &taskbar_dpi,
        position: settings.position,
        manual_offset: settings.manual_offset,
        preferred_screen_x: available_span.map(|span| span.left()),
        target_width: target_physical_width,
        animation_revision,
        latest_animation_revision,
    })?;

    Ok(measurement)
}
