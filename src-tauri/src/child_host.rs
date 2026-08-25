use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use tauri::{PhysicalSize, Runtime, Webview, Window};

use crate::platform::windows::{
    GetCurrentProcessId, GetParent, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
    IsWindow, ScreenToClient, SetLayeredWindowAttributes, SetParent, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, COLORREF, GWL_EXSTYLE, GWL_STYLE, HWND, HWND_TOP, LWA_ALPHA, POINT,
    RECT, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, WS_CHILD,
    WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::taskbar::{TaskbarDpi, TaskbarIdentity, TaskbarRect};
use crate::{settings::TaskbarPosition, taskbar_occupancy};

const STABILIZATION_DELAYS: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(300),
    Duration::from_millis(700),
];
const WIDTH_ANIMATION_STEPS: i32 = 12;
const WIDTH_ANIMATION_STEP_DURATION: Duration = Duration::from_millis(15);
const WEBVIEW_EXPANSION_LEAD_TIME: Duration = Duration::from_millis(34);

/// Child 挂载后提供给 WebView 创建流程的物理像素尺寸。
pub(crate) struct ChildHostSize {
    pub(crate) width: u32,
    pub(crate) height: u32,
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

/// 判断 Bar 当前是否已经挂载到指定任务栏，避免跨显示器切换时在旧父窗口中计算新坐标。
pub(crate) fn is_attached_to_taskbar<R: Runtime>(
    bar_window: &Window<R>,
    taskbar: &TaskbarIdentity,
) -> bool {
    let Ok(bar_handle) = bar_window.hwnd() else {
        return false;
    };
    unsafe { GetParent(bar_handle) }
        .map(|parent| parent == taskbar.handle())
        .unwrap_or(false)
}

/// 将 Tauri 窗口转换为正式 Child 宿主并挂载到目标任务栏矩形。
///
/// 调用方只需要提供经过验证的任务栏身份、矩形和目标位置；窗口样式、分层属性、
/// 父子关系、坐标转换和短期位置稳定化全部由本模块维护。
pub(crate) fn attach_window<R: Runtime>(
    bar_window: &Window<R>,
    taskbar: &TaskbarIdentity,
    taskbar_rect: &TaskbarRect,
    taskbar_dpi: &TaskbarDpi,
    position: TaskbarPosition,
    manual_offset: i32,
) -> Result<ChildHostSize, String> {
    let bar_handle = bar_window
        .hwnd()
        .map_err(|error| format!("无法取得 Bar 窗口句柄：{error}"))?;

    // 已挂载到 Explorer 的跨进程 Child 无法可靠修改扩展样式。恢复显示前先解除
    // 父子关系，使后续准备过程与首次挂载保持相同顺序，再由 attach_to_taskbar 挂回。
    let _ = unsafe { SetParent(bar_handle, None) };
    prepare_window(bar_handle)?;
    let (width, height) = attach_to_taskbar(
        bar_window,
        bar_handle,
        taskbar,
        taskbar_rect,
        taskbar_dpi,
        position,
        manual_offset,
    )?;

    Ok(ChildHostSize {
        width: u32::try_from(width).map_err(|_| "Bar 宽度无法转换为物理像素".to_owned())?,
        height: u32::try_from(height).map_err(|_| "Bar 高度无法转换为物理像素".to_owned())?,
    })
}

/// 使用 Win32 隐藏任务栏 Child，避免 Tauri 顶层窗口抽象无法同步实际可见状态。
pub(crate) fn hide_window<R: Runtime>(bar_window: &Window<R>) -> Result<(), String> {
    let bar_handle = bar_window
        .hwnd()
        .map_err(|error| format!("无法取得 Bar 窗口句柄：{error}"))?;

    // ShowWindow 返回的是调用前是否可见，不代表操作成功与否，因此只需显式消费结果。
    let _ = unsafe { ShowWindow(bar_handle, SW_HIDE) };
    Ok(())
}

/// 在保持 Child 高度不变的前提下调整原生宿主宽度与目标横坐标。
///
/// 缩短时保留平滑动画；增长时一次完成。WebView2 的合成表面晚于原生父窗口扩展，
/// 逐帧增长会反复暴露尚未绘制的新区域，形成白色拖影。
pub(crate) fn animate_window_width<R: Runtime>(
    bar_window: Window<R>,
    bar_webview: Webview<R>,
    taskbar: &TaskbarIdentity,
    taskbar_rect: &TaskbarRect,
    taskbar_dpi: &TaskbarDpi,
    position: TaskbarPosition,
    manual_offset: i32,
    preferred_screen_x: Option<i32>,
    target_width: i32,
    animation_revision: u64,
    latest_animation_revision: Arc<AtomicU64>,
) -> Result<(), String> {
    if target_width <= 0 {
        return Err("Bar 目标物理宽度必须大于零".to_owned());
    }

    let bar_handle = bar_window
        .hwnd()
        .map_err(|error| format!("无法取得 Bar 窗口句柄：{error}"))?;
    let taskbar_handle = unsafe { GetParent(bar_handle) }
        .map_err(|error| format!("无法读取 Bar 当前父窗口：{error}"))?;
    let mut bar_rect = RECT::default();
    unsafe { GetWindowRect(bar_handle, &mut bar_rect) }
        .map_err(|error| format!("无法读取 Bar 当前矩形：{error}"))?;
    let start_width = bar_rect
        .right
        .checked_sub(bar_rect.left)
        .ok_or_else(|| "Bar 当前宽度超出可表示范围".to_owned())?;
    let height = bar_rect
        .bottom
        .checked_sub(bar_rect.top)
        .ok_or_else(|| "Bar 当前高度超出可表示范围".to_owned())?;
    if start_width <= 0 || height <= 0 {
        return Err(format!("Bar 当前矩形无效：{bar_rect:?}"));
    }

    let mut client_position = POINT {
        x: bar_rect.left,
        y: bar_rect.top,
    };
    if !unsafe { ScreenToClient(taskbar_handle, &mut client_position) }.as_bool() {
        return Err("无法将 Bar 当前位置转换为任务栏客户区坐标".to_owned());
    }
    let base_screen_x = match preferred_screen_x {
        Some(screen_x) => screen_x,
        None => {
            let occupied_regions =
                taskbar_occupancy::read_positioning_regions(taskbar, taskbar_rect);
            taskbar_occupancy::resolve_bar_screen_x(
                position,
                taskbar_rect,
                &occupied_regions,
                target_width,
            )
        }
    };
    let target_screen_x = apply_manual_offset(
        base_screen_x,
        manual_offset,
        taskbar_dpi,
        taskbar_rect,
        target_width,
    );
    let mut target_client_position = POINT {
        x: target_screen_x,
        y: taskbar_rect.top(),
    };
    if !unsafe { ScreenToClient(taskbar.handle(), &mut target_client_position) }.as_bool() {
        return Err("无法将 Bar 目标位置转换为任务栏客户区坐标".to_owned());
    }
    let bar_handle_value = bar_handle.0 as usize;
    let taskbar_handle_value = taskbar_handle.0 as usize;
    let is_expanding = target_width > start_width;
    let position_distance = target_client_position.x.abs_diff(client_position.x);
    let large_reposition_threshold = u32::try_from(taskbar_rect.width() / 3).unwrap_or_default();
    let is_large_reposition = position_distance > large_reposition_threshold;
    let is_instant_change = is_expanding || is_large_reposition;
    let animation_steps = if is_instant_change {
        1
    } else {
        WIDTH_ANIMATION_STEPS
    };

    let webview_width = u32::try_from(target_width)
        .map_err(|_| "Bar WebView 目标宽度无法转换为物理像素".to_owned())?;
    let webview_height =
        u32::try_from(height).map_err(|_| "Bar WebView 高度无法转换为物理像素".to_owned())?;
    let target_webview_size = PhysicalSize::new(webview_width, webview_height);
    if is_expanding {
        // WebView 先在旧宿主的裁剪范围外完成扩展和绘制。等待约两帧后再放大宿主，
        // 用户看到新区域时合成表面已经准备好，不会暴露透明宿主的空白底色。
        bar_webview
            .set_size(target_webview_size)
            .map_err(|error| format!("无法预扩展 Bar WebView：{error}"))?;
    }

    thread::Builder::new()
        .name("muse-bar-width-animation".to_owned())
        .spawn(move || {
            if is_expanding {
                thread::sleep(WEBVIEW_EXPANSION_LEAD_TIME);
            }
            for step in 1..=animation_steps {
                if !is_instant_change {
                    thread::sleep(WIDTH_ANIMATION_STEP_DURATION);
                }
                if latest_animation_revision.load(Ordering::Acquire) != animation_revision {
                    return;
                }

                let progress = f64::from(step) / f64::from(animation_steps);
                let eased_progress = 1.0 - (1.0 - progress).powi(3);
                let width_delta = f64::from(target_width - start_width) * eased_progress;
                let width = start_width + width_delta.round() as i32;
                let x_delta = f64::from(target_client_position.x - client_position.x);
                let x = client_position.x + (x_delta * eased_progress).round() as i32;
                let scheduled_revision = Arc::clone(&latest_animation_revision);
                let final_webview = (step == animation_steps).then(|| bar_webview.clone());
                let scheduled = bar_window.run_on_main_thread(move || {
                    if scheduled_revision.load(Ordering::Acquire) != animation_revision {
                        return;
                    }

                    // HWND 不能跨线程发送，因此只传递数值，并在主线程恢复句柄类型。
                    let bar_handle = HWND(bar_handle_value as *mut _);
                    let taskbar_handle = HWND(taskbar_handle_value as *mut _);
                    let parent_matches = unsafe { GetParent(bar_handle) }
                        .map(|parent| parent == taskbar_handle)
                        .unwrap_or(false);
                    if !unsafe { IsWindow(Some(bar_handle)) }.as_bool() || !parent_matches {
                        return;
                    }

                    let resized = unsafe {
                        SetWindowPos(
                            bar_handle,
                            None,
                            x,
                            client_position.y,
                            width,
                            height,
                            SWP_NOACTIVATE | SWP_NOZORDER,
                        )
                    };
                    match resized {
                        Ok(()) => {
                            // 原生 SetWindowPos 不一定触发 Tauri 的自动 WebView 尺寸同步。
                            // 最后一帧显式对齐，避免 WebView 保留旧宽度而裁掉右侧控制按钮。
                            if let Some(webview) = final_webview {
                                if let Err(error) = webview.set_size(target_webview_size) {
                                    log::warn!("无法同步 Bar WebView 最终宽度：{error}");
                                }
                            }
                        }
                        Err(error) => log::warn!("无法调整 Bar 原生宿主宽度：{error}"),
                    }
                });
                if let Err(error) = scheduled {
                    log::warn!("无法将 Bar 宽度动画提交到主线程：{error}");
                    return;
                }
            }
        })
        .map(|_| ())
        .map_err(|error| format!("无法启动 Bar 宽度动画线程：{error}"))
}

/// 在挂载任务栏前，将 Tauri 顶层宿主转换为可交互的分层 Child 窗口。
fn prepare_window(handle: HWND) -> Result<(), String> {
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

/// 将准备完成的 Bar 挂载到任务栏，并按当前设置放入任务栏客户区。
fn attach_to_taskbar<R: Runtime>(
    bar_window: &Window<R>,
    bar_handle: HWND,
    taskbar: &TaskbarIdentity,
    taskbar_rect: &TaskbarRect,
    taskbar_dpi: &TaskbarDpi,
    position: TaskbarPosition,
    manual_offset: i32,
) -> Result<(i32, i32), String> {
    let (bar_width, bar_height) = read_window_size(bar_handle)?;

    if bar_width > taskbar_rect.width() || bar_height > taskbar_rect.height() {
        return Err(format!(
            "Bar 尺寸 {bar_width}×{bar_height} 超出任务栏 {}×{}",
            taskbar_rect.width(),
            taskbar_rect.height()
        ));
    }

    let occupied_regions = taskbar_occupancy::read_positioning_regions(taskbar, taskbar_rect);
    let base_screen_x = taskbar_occupancy::resolve_bar_screen_x(
        position,
        taskbar_rect,
        &occupied_regions,
        bar_width,
    );
    let screen_x = apply_manual_offset(
        base_screen_x,
        manual_offset,
        taskbar_dpi,
        taskbar_rect,
        bar_width,
    );
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

    Ok((bar_width, bar_height))
}

/// 把逻辑像素偏移应用到基础位置，并保证 Bar 始终留在目标任务栏内部。
fn apply_manual_offset(
    base_screen_x: i32,
    manual_offset: i32,
    taskbar_dpi: &TaskbarDpi,
    taskbar_rect: &TaskbarRect,
    bar_width: i32,
) -> i32 {
    let offset = taskbar_dpi.logical_to_physical(manual_offset);
    let minimum_x = taskbar_rect.left();
    let maximum_x = taskbar_rect
        .right()
        .saturating_sub(bar_width)
        .max(minimum_x);
    base_screen_x
        .saturating_add(offset)
        .clamp(minimum_x, maximum_x)
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

                    // 内容测量可能已启动正式宽度动画；此时不能再用创建时宽度覆盖新结果。
                    if read_window_size(bar_handle)
                        .map(|(current_width, _)| current_width != width)
                        .unwrap_or(true)
                    {
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
