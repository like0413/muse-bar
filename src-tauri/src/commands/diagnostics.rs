use std::{fs, os::windows::process::CommandExt, process::Command};

use serde::Serialize;
use tauri::{AppHandle, Manager};

const CREATE_NO_WINDOW: u32 = 0x08000000;

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

/// 一个任务栏原生控件的诊断矩形与可访问性身份。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarOccupiedRegionDiagnostic {
    name: String,
    class_name: String,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    width: i32,
    height: i32,
}

/// 任务栏占用区域的检测来源、回退原因和完整区域列表。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarOccupancyDiagnostic {
    source: String,
    fallback_reason: Option<String>,
    regions: Vec<TaskbarOccupiedRegionDiagnostic>,
}

/// 读取 Windows 自身报告的版本号，并避免诊断命令弹出控制台窗口。
#[tauri::command]
pub fn get_windows_version() -> Result<WindowsVersionDiagnostic, String> {
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

impl TaskbarOccupiedRegionDiagnostic {
    /// 将任务栏占用领域对象转换为前端可直接检查的物理像素数据。
    fn from_occupied_region(region: &crate::taskbar_occupancy::OccupiedRegion) -> Self {
        let rect = region.rect();
        Self {
            name: region.name().to_owned(),
            class_name: region.class_name().to_owned(),
            left: rect.left(),
            top: rect.top(),
            right: rect.right(),
            bottom: rect.bottom(),
            width: rect.width(),
            height: rect.height(),
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

/// 返回任务栏框架、任务按钮和系统托盘等原生控件的屏幕占用矩形。
#[tauri::command]
pub async fn get_taskbar_occupied_regions() -> Result<TaskbarOccupancyDiagnostic, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let taskbar = crate::taskbar::find_main_taskbar()?;
        let taskbar_rect = crate::taskbar::read_taskbar_rect(&taskbar)?;
        let occupancy = crate::taskbar_occupancy::read_occupied_regions(&taskbar, &taskbar_rect);

        Ok(TaskbarOccupancyDiagnostic {
            source: occupancy.source().as_str().to_owned(),
            fallback_reason: occupancy.fallback_reason().map(str::to_owned),
            regions: occupancy
                .regions()
                .iter()
                .map(TaskbarOccupiedRegionDiagnostic::from_occupied_region)
                .collect(),
        })
    })
    .await
    .map_err(|error| format!("任务栏占用区域后台任务意外终止：{error}"))?
}

/// 创建应用日志目录，并交给 Windows 文件资源管理器打开。
#[tauri::command]
pub fn open_log_directory(app: AppHandle) -> Result<(), String> {
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
}
