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

/// 查找并验证主任务栏，然后返回可序列化的诊断数据。
#[cfg(target_os = "windows")]
#[tauri::command]
pub fn get_taskbar_identity() -> Result<TaskbarIdentityDiagnostic, String> {
    let taskbar = crate::taskbar::find_main_taskbar()?;

    Ok(TaskbarIdentityDiagnostic {
        hwnd: taskbar.handle_value(),
        explorer_process_id: taskbar.explorer_process_id(),
    })
}

/// 查找主任务栏并返回它在屏幕坐标系中的物理像素矩形。
#[cfg(target_os = "windows")]
#[tauri::command]
pub fn get_taskbar_rect() -> Result<TaskbarRectDiagnostic, String> {
    let taskbar = crate::taskbar::find_main_taskbar()?;
    let rect = crate::taskbar::read_taskbar_rect(&taskbar)?;

    Ok(TaskbarRectDiagnostic {
        left: rect.left(),
        top: rect.top(),
        right: rect.right(),
        bottom: rect.bottom(),
        width: rect.width(),
        height: rect.height(),
    })
}

/// 在非 Windows 平台返回明确的不支持错误。
#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn get_taskbar_identity() -> Result<TaskbarIdentityDiagnostic, String> {
    Err("任务栏诊断仅支持 Windows".to_owned())
}

/// 在非 Windows 平台返回明确的不支持错误。
#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn get_taskbar_rect() -> Result<TaskbarRectDiagnostic, String> {
    Err("任务栏诊断仅支持 Windows".to_owned())
}
