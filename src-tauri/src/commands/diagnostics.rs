use serde::Serialize;

/// 前端诊断页可读取的主任务栏身份信息。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarIdentityDiagnostic {
    hwnd: u64,
    explorer_process_id: u32,
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

/// 在非 Windows 平台返回明确的不支持错误。
#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn get_taskbar_identity() -> Result<TaskbarIdentityDiagnostic, String> {
    Err("任务栏诊断仅支持 Windows".to_owned())
}
