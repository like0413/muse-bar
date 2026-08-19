use serde::Serialize;

/// 前端诊断页可读取的主任务栏身份信息。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarIdentityDiagnostic {
    hwnd: u64,
    explorer_process_id: u32,
}

/// 前端诊断页可读取的主任务栏物理像素矩形。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarRectDiagnostic {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    width: i32,
    height: i32,
}

/// 前端诊断页可读取的任务栏逻辑像素矩形。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalTaskbarRectDiagnostic {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    width: f64,
    height: f64,
}

/// 任务栏 DPI 以及物理像素与逻辑像素的完整换算结果。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarDpiDiagnostic {
    dpi: u32,
    scale_factor: f64,
    physical_rect: TaskbarRectDiagnostic,
    logical_rect: LogicalTaskbarRectDiagnostic,
}

impl TaskbarRectDiagnostic {
    /// 将任务栏领域矩形转换为前端诊断数据。
    fn from_taskbar_rect(rect: &crate::taskbar::TaskbarRect) -> Self {
        Self {
            left: rect.left(),
            top: rect.top(),
            right: rect.right(),
            bottom: rect.bottom(),
            width: rect.width(),
            height: rect.height(),
        }
    }
}

impl LogicalTaskbarRectDiagnostic {
    /// 根据任务栏 DPI 将物理矩形转换为逻辑像素矩形。
    fn from_physical_rect(
        rect: &crate::taskbar::TaskbarRect,
        dpi: &crate::taskbar::TaskbarDpi,
    ) -> Self {
        Self {
            left: dpi.physical_to_logical(rect.left()),
            top: dpi.physical_to_logical(rect.top()),
            right: dpi.physical_to_logical(rect.right()),
            bottom: dpi.physical_to_logical(rect.bottom()),
            width: dpi.physical_to_logical(rect.width()),
            height: dpi.physical_to_logical(rect.height()),
        }
    }
}

/// 查找并验证主任务栏，然后返回可序列化的诊断数据。
#[tauri::command]
pub fn get_taskbar_identity() -> Result<TaskbarIdentityDiagnostic, String> {
    let taskbar = crate::taskbar::find_main_taskbar()?;

    Ok(TaskbarIdentityDiagnostic {
        hwnd: taskbar.handle_value(),
        explorer_process_id: taskbar.explorer_process_id(),
    })
}

/// 查找主任务栏并返回它在屏幕坐标系中的物理像素矩形。
#[tauri::command]
pub fn get_taskbar_rect() -> Result<TaskbarRectDiagnostic, String> {
    let taskbar = crate::taskbar::find_main_taskbar()?;
    let rect = crate::taskbar::read_taskbar_rect(&taskbar)?;

    Ok(TaskbarRectDiagnostic::from_taskbar_rect(&rect))
}

/// 返回任务栏窗口 DPI 以及物理像素到逻辑像素的换算结果。
#[tauri::command]
pub fn get_taskbar_dpi() -> Result<TaskbarDpiDiagnostic, String> {
    let taskbar = crate::taskbar::find_main_taskbar()?;
    let physical_rect = crate::taskbar::read_taskbar_rect(&taskbar)?;
    let dpi = crate::taskbar::read_taskbar_dpi(&taskbar)?;

    Ok(TaskbarDpiDiagnostic {
        dpi: dpi.dpi(),
        scale_factor: dpi.scale_factor(),
        physical_rect: TaskbarRectDiagnostic::from_taskbar_rect(&physical_rect),
        logical_rect: LogicalTaskbarRectDiagnostic::from_physical_rect(&physical_rect, &dpi),
    })
}
