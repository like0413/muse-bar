use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};

use crate::platform::windows::{
    w, CloseHandle, FindWindowW, GetWindowRect, GetWindowThreadProcessId, OpenProcess,
    QueryFullProcessImageNameW, HANDLE, HWND, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PWSTR, RECT,
};

const PROCESS_PATH_BUFFER_LENGTH: usize = 32_768;

/// 经过 Explorer 进程校验的主任务栏身份。
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
    width: i32,
    height: i32,
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
        self.width
    }

    /// 返回任务栏的物理像素高度。
    pub fn height(&self) -> i32 {
        self.height
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

/// 查找主任务栏，并验证它确实属于 explorer.exe。
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

    Ok(TaskbarIdentity {
        handle,
        explorer_process_id: process_id,
    })
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
        width,
        height,
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
