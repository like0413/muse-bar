use std::{fs, os::windows::process::CommandExt, process::Command};

use serde::Serialize;
use tauri::{AppHandle, Manager};

const CREATE_NO_WINDOW: u32 = 0x08000000;

async fn run_blocking_diagnostic<T>(
    context: &'static str,
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| format!("{context}后台任务意外终止：{error}"))?
}

/// 前端诊断页显示的 Windows 产品名、版本号和构建号。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsVersionDiagnostic {
    product_name: String,
    version: String,
    build: u32,
}

/// 前端诊断页可读取的主任务栏身份信息。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarIdentityDiagnostic {
    hwnd: u64,
    explorer_process_id: u32,
}

/// 任务栏 DPI 以及前端实际展示的物理尺寸。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarDpiDiagnostic {
    dpi: u32,
    scale_factor: f64,
    physical_width: i32,
    physical_height: i32,
}

/// 任务栏占用区域的检测来源、回退原因和完整区域列表。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarOccupancyDiagnostic {
    source: String,
    fallback_reason: Option<String>,
    region_count: usize,
}

/// 读取 Windows 自身报告的版本号，并避免诊断命令弹出控制台窗口。
#[tauri::command]
pub async fn get_windows_version() -> Result<WindowsVersionDiagnostic, String> {
    run_blocking_diagnostic("Windows 版本诊断", read_windows_version).await
}

fn read_windows_version() -> Result<WindowsVersionDiagnostic, String> {
    let output = Command::new("cmd.exe")
        .args(["/D", "/C", "ver"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("无法读取 Windows 版本：{error}"))?;

    if !output.status.success() {
        return Err(format!("Windows 版本命令返回失败状态：{}", output.status));
    }

    // `ver` 的本地化文字可能不是 UTF-8，但方括号内的数字始终是 ASCII。
    let text = String::from_utf8_lossy(&output.stdout);
    let version_text = text
        .split_once('[')
        .and_then(|(_, remainder)| remainder.split_once(']'))
        .map(|(value, _)| value.trim())
        .ok_or_else(|| "Windows 版本命令返回了无法识别的内容".to_owned())?;
    let version = version_text
        .trim_start_matches(|character: char| !character.is_ascii_digit())
        .to_owned();
    let build = version
        .split('.')
        .nth(2)
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| format!("无法从版本号 {version} 中读取构建号"))?;

    Ok(WindowsVersionDiagnostic {
        product_name: if build >= 22_000 {
            "Windows 11".to_owned()
        } else {
            "Windows".to_owned()
        },
        version,
        build,
    })
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

/// 返回任务栏窗口 DPI 以及物理像素到逻辑像素的换算结果。
#[tauri::command]
pub fn get_taskbar_dpi() -> Result<TaskbarDpiDiagnostic, String> {
    let taskbar = crate::taskbar::find_main_taskbar()?;
    let physical_rect = crate::taskbar::read_taskbar_rect(&taskbar)?;
    let dpi = crate::taskbar::read_taskbar_dpi(&taskbar)?;

    Ok(TaskbarDpiDiagnostic {
        dpi: dpi.dpi(),
        scale_factor: dpi.scale_factor(),
        physical_width: physical_rect.width(),
        physical_height: physical_rect.height(),
    })
}

/// 返回任务栏框架、任务按钮和系统托盘等原生控件的屏幕占用矩形。
#[tauri::command]
pub async fn get_taskbar_occupied_regions() -> Result<TaskbarOccupancyDiagnostic, String> {
    run_blocking_diagnostic("任务栏占用区域诊断", || {
        let taskbar = crate::taskbar::find_main_taskbar()?;
        let taskbar_rect = crate::taskbar::read_taskbar_rect(&taskbar)?;
        let occupancy = crate::taskbar::read_occupied_regions(&taskbar, &taskbar_rect);

        Ok(TaskbarOccupancyDiagnostic {
            source: occupancy.source().as_str().to_owned(),
            fallback_reason: occupancy.fallback_reason().map(str::to_owned),
            region_count: occupancy.regions().len(),
        })
    })
    .await
}

/// 创建应用日志目录，并交给 Windows 文件资源管理器打开。
#[tauri::command]
pub async fn open_log_directory(app: AppHandle) -> Result<(), String> {
    run_blocking_diagnostic("日志目录操作", move || {
        let log_directory = app
            .path()
            .app_log_dir()
            .map_err(|error| format!("无法确定日志目录：{error}"))?;

        fs::create_dir_all(&log_directory).map_err(|error| format!("无法创建日志目录：{error}"))?;
        Command::new("explorer.exe")
            .arg(&log_directory)
            .spawn()
            .map_err(|error| format!("无法打开日志目录：{error}"))?;

        Ok(())
    })
    .await
}
