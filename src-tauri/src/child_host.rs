use std::{thread, time::Duration};

use tauri::{Runtime, Window};

use crate::platform::windows::{
    GetCurrentProcessId, GetParent, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
    IsWindow, ScreenToClient, SetLayeredWindowAttributes, SetParent, SetWindowLongPtrW,
    SetWindowPos, COLORREF, GWL_EXSTYLE, GWL_STYLE, HWND, HWND_TOP, LWA_ALPHA, POINT, RECT,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_SHOWWINDOW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::taskbar::{TaskbarIdentity, TaskbarRect};

const STABILIZATION_DELAYS: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(300),
    Duration::from_millis(700),
];

/// Child 挂载后提供给 WebView 创建流程的原生窗口信息。
pub(crate) struct ChildAttachment {
    pub(crate) width: i32,
    pub(crate) height: i32,
}

/// 判断 Tauri 保存的 Bar 句柄是否仍对应当前进程中的有效窗口。
pub(crate) fn is_window_alive<R: Runtime>(bar_window: &Window<R>) -> bool {
    let Ok(handle) = bar_window.hwnd() else {
        return false;
    };
    if !unsafe { IsWindow(Some(handle)) }.as_bool() {
        return false;
    }

    // HWND 数值可能在窗口销毁后被系统复用。核对进程归属可以避免修改其他程序
    // 恰好复用了同一数值的新窗口。
    let mut process_id = 0;
    let _ = unsafe { GetWindowThreadProcessId(handle, Some(&mut process_id)) };
    process_id == unsafe { GetCurrentProcessId() }
}

/// 在挂载任务栏前，将 Tauri 顶层宿主转换为可交互的分层 Child 窗口。
pub(crate) fn prepare_window<R: Runtime>(bar_window: &Window<R>) -> Result<(), String> {
    let handle = bar_window
        .hwnd()
        .map_err(|error| format!("无法取得 Bar 窗口句柄：{error}"))?;

    let current_style = read_window_style(handle)?;
    let child_style =
        (current_style & !WS_POPUP.0) | WS_CHILD.0 | WS_CLIPCHILDREN.0 | WS_CLIPSIBLINGS.0;
    let _ = unsafe { SetWindowLongPtrW(handle, GWL_STYLE, child_style as isize) };
    let applied_style = read_window_style(handle)?;
    if applied_style != child_style {
        return Err(format!(
            "Bar Child 样式未完整生效：期望 0x{child_style:08X}，实际 0x{applied_style:08X}"
        ));
    }

    let current_extended_style = read_window_extended_style(handle);
    let child_extended_style =
        (current_extended_style & !WS_EX_TRANSPARENT.0) | WS_EX_NOACTIVATE.0 | WS_EX_LAYERED.0;
    let _ = unsafe { SetWindowLongPtrW(handle, GWL_EXSTYLE, child_extended_style as isize) };

    // Windows 11 的现代任务栏要求外部窗口在 SetParent 前已经具备分层属性，
    // 否则窗口虽然可见，悬停和点击仍会由下方任务栏元素处理。
    unsafe { SetLayeredWindowAttributes(handle, COLORREF(0), 255, LWA_ALPHA) }
        .map_err(|error| format!("无法启用 Bar 分层窗口属性：{error}"))?;

    let applied_extended_style = read_window_extended_style(handle);
    if applied_extended_style != child_extended_style {
        return Err(format!(
            "Bar Child 扩展样式未完整生效：期望 0x{child_extended_style:08X}，实际 0x{applied_extended_style:08X}"
        ));
    }

    Ok(())
}

/// 将准备完成的 Bar 挂载到任务栏，并在任务栏客户区中居中放置。
pub(crate) fn attach_to_taskbar<R: Runtime>(
    bar_window: &Window<R>,
    taskbar: &TaskbarIdentity,
    taskbar_rect: &TaskbarRect,
) -> Result<ChildAttachment, String> {
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
            Some(HWND_TOP),
            client_position.x,
            client_position.y,
            bar_width,
            bar_height,
            SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
        )
    }
    .map_err(|error| format!("无法在任务栏客户区中放置 Bar：{error}"))?;

    schedule_position_stabilization(
        bar_window.clone(),
        bar_handle,
        taskbar.handle(),
        client_position,
        bar_width,
        bar_height,
    )?;

    Ok(ChildAttachment {
        width: bar_width,
        height: bar_height,
    })
}

/// 在 WebView 异步初始化期间短暂重申位置，防止顶层坐标再次写入 Child。
fn schedule_position_stabilization<R: Runtime>(
    bar_window: Window<R>,
    bar_handle: HWND,
    taskbar_handle: HWND,
    position: POINT,
    width: i32,
    height: i32,
) -> Result<(), String> {
    // HWND 本轮已经成功使用过，直接传递它的数值，避免再次向 Tauri 查询同一句柄。
    let bar_value = bar_handle.0 as usize;
    let taskbar_value = taskbar_handle.0 as usize;

    thread::Builder::new()
        .name("muse-bar-child-stabilization".to_owned())
        .spawn(move || {
            for (index, delay) in STABILIZATION_DELAYS.into_iter().enumerate() {
                thread::sleep(delay);

                let scheduled = bar_window.run_on_main_thread(move || {
                    // HWND 不能直接跨线程发送，因此只传递数值，并在主线程恢复句柄类型。
                    let bar_handle = HWND(bar_value as *mut _);
                    let taskbar_handle = HWND(taskbar_value as *mut _);
                    let parent_matches = unsafe { GetParent(bar_handle) }
                        .map(|parent| parent == taskbar_handle)
                        .unwrap_or(false);
                    if !unsafe { IsWindow(Some(bar_handle)) }.as_bool() || !parent_matches {
                        return;
                    }

                    let positioned = unsafe {
                        SetWindowPos(
                            bar_handle,
                            Some(HWND_TOP),
                            position.x,
                            position.y,
                            width,
                            height,
                            SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
                        )
                    };
                    if let Err(error) = positioned {
                        log::warn!("Bar 第 {} 次位置稳定化失败：{error}", index + 1);
                    }
                });
                if let Err(error) = scheduled {
                    log::warn!(
                        "无法将 Bar 第 {} 次位置稳定化提交到主线程：{error}",
                        index + 1
                    );
                }
            }
        })
        .map(|_| ())
        .map_err(|error| format!("无法启动 Bar 位置稳定化线程：{error}"))
}

/// 读取 HWND 当前的普通窗口样式。
fn read_window_style(handle: HWND) -> Result<u32, String> {
    let style = unsafe { GetWindowLongPtrW(handle, GWL_STYLE) } as u32;
    if style == 0 {
        return Err("无法读取 Bar 窗口样式".to_owned());
    }

    Ok(style)
}

/// 读取 HWND 当前的扩展窗口样式。
fn read_window_extended_style(handle: HWND) -> u32 {
    unsafe { GetWindowLongPtrW(handle, GWL_EXSTYLE) as u32 }
}

/// 读取 Bar 挂载前的物理像素尺寸。
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
