use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};

use crate::platform::windows::{
    w, CloseHandle, FindWindowW, GetWindowThreadProcessId, OpenProcess, QueryFullProcessImageNameW,
    HANDLE, HWND, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PWSTR,
};

const PROCESS_PATH_BUFFER_LENGTH: usize = 32_768;

/// 经过 Explorer 进程校验的主任务栏身份。
#[derive(Debug)]
pub struct TaskbarIdentity {
    handle: HWND,
    explorer_process_id: u32,
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
