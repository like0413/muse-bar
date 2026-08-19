use std::thread;

use crate::{
    platform::windows::{
        CUIAutomation, CoCreateInstance, CoInitializeEx, CoUninitialize, EnumChildWindows,
        GetClassNameW, GetWindowRect, GetWindowThreadProcessId, IUIAutomation, IsWindowVisible,
        TreeScope_Descendants, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, LPARAM, RECT,
    },
    settings::TaskbarPosition,
    taskbar::{TaskbarIdentity, TaskbarRect},
};

const EDGE_COMPONENT_ZONE_DIVISOR: i32 = 3;

/// 任务栏占用区域的检测来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OccupancySource {
    UiAutomation,
    Win32Fallback,
}

impl OccupancySource {
    /// 返回可稳定传递给前端的来源标识。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UiAutomation => "uiAutomation",
            Self::Win32Fallback => "win32Fallback",
        }
    }
}

/// 一个任务栏控件在屏幕物理像素坐标系中的矩形。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OccupiedRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl OccupiedRect {
    /// 返回矩形左边界。
    pub fn left(self) -> i32 {
        self.left
    }

    /// 返回矩形上边界。
    pub fn top(self) -> i32 {
        self.top
    }

    /// 返回矩形右边界。
    pub fn right(self) -> i32 {
        self.right
    }

    /// 返回矩形下边界。
    pub fn bottom(self) -> i32 {
        self.bottom
    }

    /// 返回矩形宽度。
    pub fn width(self) -> i32 {
        self.right - self.left
    }

    /// 返回矩形高度。
    pub fn height(self) -> i32 {
        self.bottom - self.top
    }
}

/// 一个可用于后续避让计算的任务栏控件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccupiedRegion {
    name: String,
    class_name: String,
    rect: OccupiedRect,
}

impl OccupiedRegion {
    /// 返回 UI Automation 名称或 Win32 回退名称。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 返回控件类名，供诊断不同 Windows 版本的任务栏结构。
    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    /// 返回控件的屏幕物理像素矩形。
    pub fn rect(&self) -> OccupiedRect {
        self.rect
    }
}

/// 一次任务栏占用区域读取的完整结果。
#[derive(Debug)]
pub struct TaskbarOccupancy {
    source: OccupancySource,
    regions: Vec<OccupiedRegion>,
    fallback_reason: Option<String>,
}

impl TaskbarOccupancy {
    /// 返回本次实际采用的检测来源。
    pub fn source(&self) -> OccupancySource {
        self.source
    }

    /// 返回已经过滤和去重的占用区域。
    pub fn regions(&self) -> &[OccupiedRegion] {
        &self.regions
    }

    /// UI Automation 失败并进入回退时，返回原始失败原因。
    pub fn fallback_reason(&self) -> Option<&str> {
        self.fallback_reason.as_deref()
    }
}

/// 优先使用 UI Automation 读取任务栏控件；失败或无结果时退回 Win32 子窗口枚举。
pub fn read_occupied_regions(
    taskbar: &TaskbarIdentity,
    taskbar_rect: &TaskbarRect,
) -> TaskbarOccupancy {
    match read_ui_automation_regions(taskbar, taskbar_rect) {
        Ok(regions) if !regions.is_empty() => TaskbarOccupancy {
            source: OccupancySource::UiAutomation,
            regions,
            fallback_reason: None,
        },
        result => {
            let fallback_reason = match result {
                Ok(_) => "UI Automation 没有返回可用的 Explorer 任务栏控件".to_owned(),
                Err(error) => error,
            };
            let regions = read_win32_regions(taskbar, taskbar_rect);
            TaskbarOccupancy {
                source: OccupancySource::Win32Fallback,
                regions,
                fallback_reason: Some(fallback_reason),
            }
        }
    }
}

/// 为频繁定位返回轻量 Win32 区域，避免 UI Automation 在任务栏内反查 Muse Bar WebView。
pub fn read_positioning_regions(
    taskbar: &TaskbarIdentity,
    taskbar_rect: &TaskbarRect,
) -> Vec<OccupiedRegion> {
    read_win32_regions(taskbar, taskbar_rect)
}

/// 按简化规则计算 Bar 的屏幕横坐标：居中直接覆盖，左右只跟随对应边缘组件。
pub fn resolve_bar_screen_x(
    position: TaskbarPosition,
    taskbar_rect: &TaskbarRect,
    regions: &[OccupiedRegion],
    bar_width: i32,
) -> i32 {
    let minimum_x = taskbar_rect.left();
    let maximum_x = taskbar_rect.right().saturating_sub(bar_width);
    let taskbar_width = taskbar_rect.width();
    let left_zone_end = taskbar_rect
        .left()
        .saturating_add(taskbar_width / EDGE_COMPONENT_ZONE_DIVISOR);
    let right_zone_start = taskbar_rect
        .right()
        .saturating_sub(taskbar_width / EDGE_COMPONENT_ZONE_DIVISOR);

    let target_x = match position {
        TaskbarPosition::Center => taskbar_rect
            .left()
            .saturating_add((taskbar_width - bar_width) / 2),
        TaskbarPosition::Left => regions
            .iter()
            .filter(|region| region.rect().right() <= left_zone_end)
            .map(|region| region.rect().right())
            .max()
            .unwrap_or(taskbar_rect.left()),
        TaskbarPosition::Right => regions
            .iter()
            .filter(|region| region.rect().left() >= right_zone_start)
            .map(|region| region.rect().left())
            .min()
            .unwrap_or(taskbar_rect.right())
            .saturating_sub(bar_width),
    };

    target_x.clamp(minimum_x, maximum_x.max(minimum_x))
}

/// 在独立 MTA 线程中执行 UI Automation，避免与 Tauri 主线程的 COM 模型冲突。
fn read_ui_automation_regions(
    taskbar: &TaskbarIdentity,
    taskbar_rect: &TaskbarRect,
) -> Result<Vec<OccupiedRegion>, String> {
    let taskbar_handle = taskbar.handle_value() as usize;
    let explorer_process_id = taskbar.explorer_process_id();
    let taskbar_bounds = occupied_rect_from_taskbar(taskbar_rect);
    let worker = thread::Builder::new()
        .name("muse-bar-taskbar-uia".to_owned())
        .spawn(move || {
            read_ui_automation_regions_on_worker(
                taskbar_handle,
                explorer_process_id,
                taskbar_bounds,
            )
        })
        .map_err(|error| format!("无法启动任务栏 UI Automation 线程：{error}"))?;

    worker
        .join()
        .map_err(|_| "任务栏 UI Automation 线程意外终止".to_owned())?
}

/// 初始化 COM 并枚举任务栏可访问性树中的 Explorer 元素。
fn read_ui_automation_regions_on_worker(
    taskbar_handle: usize,
    explorer_process_id: u32,
    taskbar_bounds: OccupiedRect,
) -> Result<Vec<OccupiedRegion>, String> {
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    initialized
        .ok()
        .map_err(|error| format!("无法初始化 UI Automation COM 线程：{error}"))?;
    let _com_guard = ComGuard;

    let automation: IUIAutomation = unsafe {
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| format!("无法创建 UI Automation：{error}"))?
    };
    let taskbar_element = unsafe {
        automation
            .ElementFromHandle(crate::platform::windows::HWND(taskbar_handle as *mut _))
            .map_err(|error| format!("无法从任务栏句柄创建 UI Automation 元素：{error}"))?
    };
    let condition = unsafe {
        automation
            .CreateTrueCondition()
            .map_err(|error| format!("无法创建 UI Automation 查询条件：{error}"))?
    };
    let elements = unsafe {
        taskbar_element
            .FindAll(TreeScope_Descendants, &condition)
            .map_err(|error| format!("无法枚举任务栏 UI Automation 元素：{error}"))?
    };
    let element_count = unsafe {
        elements
            .Length()
            .map_err(|error| format!("无法读取任务栏 UI Automation 元素数量：{error}"))?
    };
    let mut regions = Vec::new();

    for index in 0..element_count {
        let Ok(element) = (unsafe { elements.GetElement(index) }) else {
            continue;
        };
        let process_matches = unsafe { element.CurrentProcessId() }
            .ok()
            .and_then(|process_id| u32::try_from(process_id).ok())
            .is_some_and(|process_id| process_id == explorer_process_id);
        if !process_matches {
            continue;
        }
        if unsafe { element.CurrentIsOffscreen() }
            .map(|is_offscreen| is_offscreen.as_bool())
            .unwrap_or(true)
        {
            continue;
        }

        let Ok(native_rect) = (unsafe { element.CurrentBoundingRectangle() }) else {
            continue;
        };
        let Some(rect) = clip_native_rect(native_rect, taskbar_bounds) else {
            continue;
        };
        if covers_entire_taskbar(rect, taskbar_bounds) {
            continue;
        }

        let name = unsafe { element.CurrentName() }
            .map(|value| value.to_string())
            .unwrap_or_default();
        let class_name = unsafe { element.CurrentClassName() }
            .map(|value| value.to_string())
            .unwrap_or_default();
        if name.is_empty() && class_name.is_empty() {
            continue;
        }

        regions.push(OccupiedRegion {
            name,
            class_name,
            rect,
        });
    }

    Ok(normalize_regions(regions))
}

/// 枚举任务栏的可见 Explorer 子窗口，作为 UI Automation 不可用时的兼容回退。
fn read_win32_regions(
    taskbar: &TaskbarIdentity,
    taskbar_rect: &TaskbarRect,
) -> Vec<OccupiedRegion> {
    let mut context = Win32EnumerationContext {
        explorer_process_id: taskbar.explorer_process_id(),
        taskbar_bounds: occupied_rect_from_taskbar(taskbar_rect),
        regions: Vec::new(),
    };
    let context_pointer = &mut context as *mut Win32EnumerationContext;
    let _ = unsafe {
        EnumChildWindows(
            Some(taskbar.handle()),
            Some(collect_win32_child),
            LPARAM(context_pointer as isize),
        )
    };

    normalize_regions(context.regions)
}

/// 接收 EnumChildWindows 回调，并把满足条件的窗口加入当前枚举结果。
unsafe extern "system" fn collect_win32_child(
    window: crate::platform::windows::HWND,
    parameter: LPARAM,
) -> crate::platform::windows::BOOL {
    let context = unsafe { &mut *(parameter.0 as *mut Win32EnumerationContext) };
    if !unsafe { IsWindowVisible(window) }.as_bool() {
        return true.into();
    }

    let mut process_id = 0;
    let _ = unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    if process_id != context.explorer_process_id {
        return true.into();
    }

    let mut native_rect = RECT::default();
    if unsafe { GetWindowRect(window, &mut native_rect) }.is_err() {
        return true.into();
    }
    let Some(rect) = clip_native_rect(native_rect, context.taskbar_bounds) else {
        return true.into();
    };
    if covers_entire_taskbar(rect, context.taskbar_bounds) {
        return true.into();
    }

    let mut class_name_buffer = [0_u16; 256];
    let class_name_length = unsafe { GetClassNameW(window, &mut class_name_buffer) };
    let class_name = if class_name_length > 0 {
        String::from_utf16_lossy(&class_name_buffer[..class_name_length as usize])
    } else {
        String::new()
    };
    context.regions.push(OccupiedRegion {
        name: class_name.clone(),
        class_name,
        rect,
    });

    true.into()
}

/// 将原生矩形裁剪到任务栏范围，完全不相交或无面积时返回空。
fn clip_native_rect(native_rect: RECT, taskbar_bounds: OccupiedRect) -> Option<OccupiedRect> {
    let rect = OccupiedRect {
        left: native_rect.left.max(taskbar_bounds.left),
        top: native_rect.top.max(taskbar_bounds.top),
        right: native_rect.right.min(taskbar_bounds.right),
        bottom: native_rect.bottom.min(taskbar_bounds.bottom),
    };
    (rect.width() > 0 && rect.height() > 0).then_some(rect)
}

/// 排除覆盖整个任务栏的框架容器，避免后续误判为没有任何可用空间。
fn covers_entire_taskbar(rect: OccupiedRect, taskbar_bounds: OccupiedRect) -> bool {
    rect.width() >= taskbar_bounds.width().saturating_sub(2)
        && rect.height() >= taskbar_bounds.height().saturating_sub(2)
}

/// 按横坐标排序并删除完全相同的矩形，保留更有名称信息的元素。
fn normalize_regions(mut regions: Vec<OccupiedRegion>) -> Vec<OccupiedRegion> {
    regions.sort_by(|left, right| {
        left.rect
            .left
            .cmp(&right.rect.left)
            .then(left.rect.right.cmp(&right.rect.right))
            .then(right.name.len().cmp(&left.name.len()))
    });
    regions.dedup_by(|right, left| right.rect == left.rect);
    regions
}

/// 将已验证任务栏矩形转换为本模块的通用占用矩形。
fn occupied_rect_from_taskbar(taskbar_rect: &TaskbarRect) -> OccupiedRect {
    OccupiedRect {
        left: taskbar_rect.left(),
        top: taskbar_rect.top(),
        right: taskbar_rect.right(),
        bottom: taskbar_rect.bottom(),
    }
}

/// 确保成功初始化的 COM 公寓在线程退出前成对释放。
struct ComGuard;

impl Drop for ComGuard {
    /// 释放当前 UI Automation 工作线程的 COM 初始化计数。
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// EnumChildWindows 回调期间共享的只读边界和可变结果集合。
struct Win32EnumerationContext {
    explorer_process_id: u32,
    taskbar_bounds: OccupiedRect,
    regions: Vec<OccupiedRegion>,
}
