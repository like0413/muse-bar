use tauri::{Runtime, WebviewWindow};

use crate::platform::windows::{
    GetParent, GetWindowLongPtrW, GetWindowRect, ScreenToClient, SetParent, SetWindowLongPtrW,
    SetWindowPos, GWL_STYLE, HWND, POINT, RECT, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOZORDER,
    SWP_SHOWWINDOW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_POPUP,
};

use crate::taskbar::{TaskbarIdentity, TaskbarRect};

/// Child 样式技术验证的修改前后快照。
#[derive(Debug, Clone, Copy)]
pub struct WindowStyleSnapshot {
    pub before: u32,
    pub requested: u32,
    pub applied: u32,
}

/// Child 挂载后的父窗口和客户区位置快照。
#[derive(Debug, Clone, Copy)]
pub struct ChildAttachmentSnapshot {
    pub parent: u64,
    pub client_x: i32,
    pub client_y: i32,
    pub width: i32,
    pub height: i32,
}

/// 将 Bar 从顶层 Popup 样式修改为尚未挂载父窗口的 Child 样式。
pub fn apply_child_style<R: Runtime>(
    bar_window: &WebviewWindow<R>,
) -> Result<WindowStyleSnapshot, String> {
    let handle = bar_window
        .hwnd()
        .map_err(|error| format!("无法取得 Bar 窗口句柄：{error}"))?;
    let before = read_window_style(handle)?;
    let requested = (before & !WS_POPUP.0) | WS_CHILD.0 | WS_CLIPCHILDREN.0 | WS_CLIPSIBLINGS.0;

    let _ = unsafe { SetWindowLongPtrW(handle, GWL_STYLE, requested as isize) };
    let applied = read_window_style(handle)?;
    if applied != requested {
        return Err(format!(
            "Bar 窗口样式修改未完整生效：期望 0x{requested:08X}，实际 0x{applied:08X}"
        ));
    }

    Ok(WindowStyleSnapshot {
        before,
        requested,
        applied,
    })
}

/// 将 Bar 挂载到任务栏，并在任务栏客户区中居中放置。
pub fn attach_to_taskbar<R: Runtime>(
    bar_window: &WebviewWindow<R>,
    taskbar: &TaskbarIdentity,
    taskbar_rect: &TaskbarRect,
) -> Result<ChildAttachmentSnapshot, String> {
    let bar_handle = bar_window
        .hwnd()
        .map_err(|error| format!("无法取得 Bar 窗口句柄：{error}"))?;
    let (bar_width, bar_height) = read_window_size(bar_handle)?;

    if bar_width > taskbar_rect.width() || bar_height > taskbar_rect.height() {
        return Err(format!(
            "Bar 尺寸 {bar_width}×{bar_height} 超出任务栏 {}×{}",
            taskbar_rect.width(),
            taskbar_rect.height()
        ));
    }

    let screen_x = taskbar_rect
        .left()
        .checked_add((taskbar_rect.width() - bar_width) / 2)
        .ok_or_else(|| "Bar 屏幕横坐标超出可表示范围".to_owned())?;
    let screen_y = taskbar_rect
        .top()
        .checked_add((taskbar_rect.height() - bar_height) / 2)
        .ok_or_else(|| "Bar 屏幕纵坐标超出可表示范围".to_owned())?;
    let mut client_position = POINT {
        x: screen_x,
        y: screen_y,
    };

    let converted = unsafe { ScreenToClient(taskbar.handle(), &mut client_position) };
    if !converted.as_bool() {
        return Err("无法将 Bar 屏幕坐标转换为任务栏客户区坐标".to_owned());
    }

    let _ = unsafe { SetParent(bar_handle, Some(taskbar.handle())) };
    let actual_parent = unsafe { GetParent(bar_handle) }
        .map_err(|error| format!("无法验证 Bar 父窗口：{error}"))?;
    if actual_parent != taskbar.handle() {
        return Err(format!(
            "Bar 未挂载到目标任务栏：期望 0x{:X}，实际 0x{:X}",
            taskbar.handle_value(),
            actual_parent.0 as usize as u64
        ));
    }

    unsafe {
        SetWindowPos(
            bar_handle,
            None,
            client_position.x,
            client_position.y,
            bar_width,
            bar_height,
            SWP_NOACTIVATE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
        )
    }
    .map_err(|error| format!("无法在任务栏客户区中放置 Bar：{error}"))?;

    Ok(ChildAttachmentSnapshot {
        parent: actual_parent.0 as usize as u64,
        client_x: client_position.x,
        client_y: client_position.y,
        width: bar_width,
        height: bar_height,
    })
}

/// 读取 HWND 当前的 32 位普通窗口样式。
fn read_window_style(handle: HWND) -> Result<u32, String> {
    let style = unsafe { GetWindowLongPtrW(handle, GWL_STYLE) } as u32;
    if style == 0 {
        return Err("无法读取 Bar 窗口样式".to_owned());
    }

    Ok(style)
}

/// 读取 Bar 在挂载前的物理像素宽高。
fn read_window_size(handle: HWND) -> Result<(i32, i32), String> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(handle, &mut rect) }
        .map_err(|error| format!("无法读取 Bar 窗口矩形：{error}"))?;

    let width = rect
        .right
        .checked_sub(rect.left)
        .ok_or_else(|| "Bar 宽度超出可表示范围".to_owned())?;
    let height = rect
        .bottom
        .checked_sub(rect.top)
        .ok_or_else(|| "Bar 高度超出可表示范围".to_owned())?;
    if width <= 0 || height <= 0 {
        return Err(format!("Bar 窗口矩形无效：{rect:?}"));
    }

    Ok((width, height))
}
