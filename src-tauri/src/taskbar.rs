use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};

use crate::platform::windows::{
    w, CloseHandle, FindWindowW, GetMonitorInfoW, GetWindowRect, GetWindowThreadProcessId,
    MonitorFromWindow, OpenProcess, QueryFullProcessImageNameW, HANDLE, HWND, MONITORINFO,
    MONITORINFOF_PRIMARY, MONITOR_DEFAULTTONULL, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PWSTR, RECT,
};

const PROCESS_PATH_BUFFER_LENGTH: usize = 32_768;
const WINDOWS_DEFAULT_DPI: f64 = 96.0;

/// 经过 Explorer 进程和主显示器双重校验的主任务栏身份。
#[derive(Debug)]
pub struct TaskbarIdentity {
    handle: HWND,
    explorer_process_id: u32,
}

/// 主任务栏在屏幕坐标系中的物理像素矩形。
#[derive(Debug, Clone, Copy)]
pub struct TaskbarRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// 任务栏窗口自身的 DPI 与对应缩放因子。
#[derive(Debug, Clone, Copy)]
pub struct TaskbarDpi {
    dpi: u32,
    scale_factor: f64,
}

impl TaskbarDpi {
    /// 返回任务栏窗口当前使用的 DPI。
    pub fn dpi(&self) -> u32 {
        self.dpi
    }

    /// 返回以 96 DPI 为基准的窗口缩放因子。
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// 将一个物理像素坐标或长度转换为逻辑像素。
    pub fn physical_to_logical(&self, physical_pixels: i32) -> f64 {
        f64::from(physical_pixels) / self.scale_factor
    }
}

impl TaskbarRect {
    /// 返回矩形左边界的屏幕横坐标。
    pub fn left(&self) -> i32 {
        self.left
    }

    /// 返回矩形上边界的屏幕纵坐标。
    pub fn top(&self) -> i32 {
        self.top
    }

    /// 返回矩形右边界的屏幕横坐标。
    pub fn right(&self) -> i32 {
        self.right
    }

    /// 返回矩形下边界的屏幕纵坐标。
    pub fn bottom(&self) -> i32 {
        self.bottom
    }

    /// 返回任务栏的物理像素宽度。
    pub fn width(&self) -> i32 {
        // 矩形只在构造时验证一次，之后由边界即时计算尺寸，避免保存两份可能不一致的数据。
        self.right - self.left
    }

    /// 返回任务栏的物理像素高度。
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

impl TaskbarIdentity {
    /// 返回后续任务栏测量需要使用的原生窗口句柄。
    pub fn handle(&self) -> HWND {
        self.handle
    }

    /// 返回可安全序列化到诊断界面的窗口句柄数值。
    pub fn handle_value(&self) -> u64 {
        self.handle.0 as usize as u64
    }

    /// 返回已验证为 Explorer 的任务栏所属进程 ID。
    pub fn explorer_process_id(&self) -> u32 {
        self.explorer_process_id
    }
}

/// 拥有一个必须在离开作用域时关闭的 Windows 句柄。
struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    /// 释放进程查询句柄，忽略已经无法补救的关闭错误。
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// 查找主任务栏，并验证它属于 explorer.exe 和 Windows 主显示器。
pub fn find_main_taskbar() -> Result<TaskbarIdentity, String> {
    let handle = unsafe { FindWindowW(w!("Shell_TrayWnd"), None) }
        .map_err(|error| format!("无法找到主任务栏窗口：{error}"))?;

    let mut process_id = 0;
    let thread_id = unsafe { GetWindowThreadProcessId(handle, Some(&mut process_id)) };
    if thread_id == 0 || process_id == 0 {
        return Err("无法读取主任务栏所属进程".to_owned());
    }

    let process_path = query_process_image_path(process_id)?;
    let is_explorer = process_path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("explorer.exe"));
    if !is_explorer {
        return Err(format!(
            "Shell_TrayWnd 不属于 explorer.exe：{}",
            process_path.display()
        ));
    }

    verify_primary_monitor(handle)?;

    Ok(TaskbarIdentity {
        handle,
        explorer_process_id: process_id,
    })
}

/// 验证任务栏 HWND 所在显示器带有 Windows 主显示器标志。
fn verify_primary_monitor(taskbar_handle: HWND) -> Result<(), String> {
    let monitor = unsafe { MonitorFromWindow(taskbar_handle, MONITOR_DEFAULTTONULL) };
    if monitor.0.is_null() {
        return Err("无法确定主任务栏所在的显示器".to_owned());
    }

    let mut monitor_info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let read_succeeded = unsafe { GetMonitorInfoW(monitor, &mut monitor_info) };
    if !read_succeeded.as_bool() {
        return Err("无法读取主任务栏所在显示器的信息".to_owned());
    }

    if monitor_info.dwFlags & MONITORINFOF_PRIMARY == 0 {
        return Err("Shell_TrayWnd 不在 Windows 主显示器上".to_owned());
    }

    Ok(())
}

/// 读取已验证任务栏的屏幕矩形，并计算物理像素宽高。
pub fn read_taskbar_rect(taskbar: &TaskbarIdentity) -> Result<TaskbarRect, String> {
    let mut native_rect = RECT::default();
    unsafe { GetWindowRect(taskbar.handle(), &mut native_rect) }
        .map_err(|error| format!("无法读取主任务栏矩形：{error}"))?;

    let width = native_rect
        .right
        .checked_sub(native_rect.left)
        .ok_or_else(|| "主任务栏宽度超出可表示范围".to_owned())?;
    let height = native_rect
        .bottom
        .checked_sub(native_rect.top)
        .ok_or_else(|| "主任务栏高度超出可表示范围".to_owned())?;
    if width <= 0 || height <= 0 {
        return Err(format!("主任务栏矩形无效：{native_rect:?}"));
    }

    Ok(TaskbarRect {
        left: native_rect.left,
        top: native_rect.top,
        right: native_rect.right,
        bottom: native_rect.bottom,
    })
}

/// 使用任务栏窗口自身的句柄读取 DPI，而不是读取全局系统 DPI。
pub fn read_taskbar_dpi(taskbar: &TaskbarIdentity) -> Result<TaskbarDpi, String> {
    let dpi = unsafe { crate::platform::windows::GetDpiForWindow(taskbar.handle()) };
    if dpi == 0 {
        return Err("无法读取主任务栏 DPI".to_owned());
    }

    Ok(TaskbarDpi {
        dpi,
        scale_factor: f64::from(dpi) / WINDOWS_DEFAULT_DPI,
    })
}

/// 使用最小查询权限读取指定进程的完整可执行文件路径。
fn query_process_image_path(process_id: u32) -> Result<PathBuf, String> {
    let process_handle = unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id)
            .map(OwnedHandle)
            .map_err(|error| format!("无法打开任务栏所属进程 {process_id}：{error}"))?
    };

    let mut path_buffer = vec![0_u16; PROCESS_PATH_BUFFER_LENGTH];
    let mut path_length = path_buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process_handle.0,
            PROCESS_NAME_WIN32,
            PWSTR(path_buffer.as_mut_ptr()),
            &mut path_length,
        )
        .map_err(|error| format!("无法读取任务栏所属进程路径：{error}"))?;
    }

    path_buffer.truncate(path_length as usize);
    Ok(PathBuf::from(OsString::from_wide(&path_buffer)))
}
