use std::{
    sync::{Mutex, OnceLock},
    thread,
};

use crate::{
    platform::windows::{
        CUIAutomation, CoCreateInstance, CoInitializeEx, CoUninitialize, EnumChildWindows,
        GetClassNameW, GetWindowRect, GetWindowThreadProcessId, IUIAutomation, IsWindowVisible,
        TreeScope_Descendants, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, HWND, LPARAM, RECT,
    },
    taskbar::{TaskbarIdentity, TaskbarRect},
    taskbar_layout::{is_central_taskbar_button, occupied_rect_from_taskbar},
};

const XAML_HOST_CLASS_NAME: &str = "Windows.UI.Composition.DesktopWindowContentBridge";

pub use crate::taskbar_layout::{
    resolve_available_span, resolve_bar_screen_x, OccupiedRect, OccupiedRegion,
};

static LAST_POSITIONING_REGIONS: OnceLock<Mutex<Option<CachedPositioningRegions>>> =
    OnceLock::new();

/// 最近一次完整的 Explorer XAML 布局，用于跨过开始菜单打开时的短暂数据缺失。
#[derive(Debug, Clone)]
struct CachedPositioningRegions {
    taskbar_handle: u64,
    taskbar_bounds: OccupiedRect,
    regions: Vec<OccupiedRegion>,
}

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

/// 从 Explorer 的 XAML 宿主读取任务栏控件，失败时退回 Win32 子窗口。
pub fn read_occupied_regions(
    taskbar: &TaskbarIdentity,
    taskbar_rect: &TaskbarRect,
) -> TaskbarOccupancy {
    match read_xaml_host_regions(taskbar, taskbar_rect) {
        Ok(regions) if regions.iter().any(is_central_taskbar_button) => TaskbarOccupancy {
            source: OccupancySource::UiAutomation,
            regions,
            fallback_reason: None,
        },
        result => TaskbarOccupancy {
            source: OccupancySource::Win32Fallback,
            regions: read_win32_regions(taskbar, taskbar_rect),
            fallback_reason: Some(match result {
                Ok(_) => "Explorer XAML 宿主尚未返回完整的中央任务按钮".to_owned(),
                Err(error) => error,
            }),
        },
    }
}

/// 为 Bar 定位返回任务栏控件；XAML 宿主与 Muse Bar 是兄弟窗口，不会进入其 WebView。
pub fn read_positioning_regions(
    taskbar: &TaskbarIdentity,
    taskbar_rect: &TaskbarRect,
) -> Vec<OccupiedRegion> {
    let taskbar_bounds = occupied_rect_from_taskbar(taskbar_rect);
    let regions = read_xaml_host_regions(taskbar, taskbar_rect)
        .ok()
        .filter(|regions| regions.iter().any(is_central_taskbar_button));
    if let Some(regions) = regions {
        if let Ok(mut cached) = positioning_regions_cache().lock() {
            *cached = Some(CachedPositioningRegions {
                taskbar_handle: taskbar.handle_value(),
                taskbar_bounds,
                regions: regions.clone(),
            });
        }
        return regions;
    }

    // 开始菜单打开、Explorer 正在动画或锁屏恢复时，XAML 树可能短暂不完整。
    // 同一任务栏上继续使用最近的可信结果，避免退回粗略容器后覆盖开始按钮。
    if let Ok(cached) = positioning_regions_cache().lock() {
        if let Some(cached) = cached.as_ref().filter(|cached| {
            cached.taskbar_handle == taskbar.handle_value()
                && cached.taskbar_bounds == taskbar_bounds
        }) {
            return cached.regions.clone();
        }
    }

    read_win32_regions(taskbar, taskbar_rect)
}

/// 返回进程内唯一的任务栏定位缓存。
fn positioning_regions_cache() -> &'static Mutex<Option<CachedPositioningRegions>> {
    LAST_POSITIONING_REGIONS.get_or_init(|| Mutex::new(None))
}

/// Explorer 重建或应用退出时丢弃旧 XAML 布局，避免句柄复用后命中过期缓存。
pub(crate) fn invalidate_positioning_regions_cache() {
    if let Ok(mut cached) = positioning_regions_cache().lock() {
        *cached = None;
    }
}

/// 在 Explorer 任务栏内部查找承载 Windows 11 XAML 控件的独立窗口。
fn find_xaml_host(taskbar: &TaskbarIdentity) -> Result<HWND, String> {
    let mut context = XamlHostSearchContext {
        explorer_process_id: taskbar.explorer_process_id(),
        handle: None,
    };
    let context_pointer = &mut context as *mut XamlHostSearchContext;
    let _ = unsafe {
        EnumChildWindows(
            Some(taskbar.handle()),
            Some(collect_xaml_host),
            LPARAM(context_pointer as isize),
        )
    };

    context
        .handle
        .ok_or_else(|| "无法找到 Explorer 任务栏 XAML 宿主".to_owned())
}

/// 接收子窗口枚举结果，并在找到属于 Explorer 的 XAML 宿主后立即停止。
unsafe extern "system" fn collect_xaml_host(
    window: HWND,
    parameter: LPARAM,
) -> crate::platform::windows::BOOL {
    // SAFETY: parameter 指向 find_xaml_host 栈上的上下文，枚举调用返回前始终有效。
    let context = unsafe { &mut *(parameter.0 as *mut XamlHostSearchContext) };
    let mut process_id = 0;
    let _ = unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    if process_id != context.explorer_process_id {
        return true.into();
    }

    let mut class_name_buffer = [0_u16; 256];
    let class_name_length = unsafe { GetClassNameW(window, &mut class_name_buffer) };
    let class_name =
        String::from_utf16_lossy(&class_name_buffer[..class_name_length.max(0) as usize]);
    if class_name == XAML_HOST_CLASS_NAME {
        context.handle = Some(window);
        return false.into();
    }

    true.into()
}

/// 在独立 MTA 线程中读取 Explorer XAML 宿主，避免影响 Tauri 事件线程的 COM 模型。
fn read_xaml_host_regions(
    taskbar: &TaskbarIdentity,
    taskbar_rect: &TaskbarRect,
) -> Result<Vec<OccupiedRegion>, String> {
    let xaml_host = find_xaml_host(taskbar)?;
    let xaml_host_handle = xaml_host.0 as usize;
    let explorer_process_id = taskbar.explorer_process_id();
    let taskbar_bounds = occupied_rect_from_taskbar(taskbar_rect);
    let worker = thread::Builder::new()
        .name("muse-bar-taskbar-xaml-uia".to_owned())
        .spawn(move || {
            read_xaml_host_regions_on_worker(xaml_host_handle, explorer_process_id, taskbar_bounds)
        })
        .map_err(|error| format!("无法启动任务栏 XAML 读取线程：{error}"))?;

    worker
        .join()
        .map_err(|_| "任务栏 XAML 读取线程意外终止".to_owned())?
}

/// 初始化 COM，并只枚举 Explorer XAML 宿主中的任务栏控件。
fn read_xaml_host_regions_on_worker(
    xaml_host_handle: usize,
    explorer_process_id: u32,
    taskbar_bounds: OccupiedRect,
) -> Result<Vec<OccupiedRegion>, String> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
        .ok()
        .map_err(|error| format!("无法初始化 UI Automation COM 线程：{error}"))?;
    let _com_guard = ComGuard;

    let automation: IUIAutomation = unsafe {
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| format!("无法创建 UI Automation：{error}"))?
    };
    let xaml_host_element = unsafe {
        automation
            .ElementFromHandle(HWND(xaml_host_handle as *mut _))
            .map_err(|error| format!("无法读取 Explorer XAML 宿主：{error}"))?
    };
    let condition = unsafe {
        automation
            .CreateTrueCondition()
            .map_err(|error| format!("无法创建 UI Automation 查询条件：{error}"))?
    };
    let elements = unsafe {
        xaml_host_element
            .FindAll(TreeScope_Descendants, &condition)
            .map_err(|error| format!("无法枚举 Explorer XAML 控件：{error}"))?
    };
    let element_count = unsafe {
        elements
            .Length()
            .map_err(|error| format!("无法读取 Explorer XAML 控件数量：{error}"))?
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

/// 确保成功初始化的 COM 公寓在线程结束前成对释放。
struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// 子窗口枚举期间保存目标 Explorer 进程和已经找到的 XAML 宿主。
struct XamlHostSearchContext {
    explorer_process_id: u32,
    handle: Option<HWND>,
}

/// EnumChildWindows 回调期间共享的只读边界和可变结果集合。
struct Win32EnumerationContext {
    explorer_process_id: u32,
    taskbar_bounds: OccupiedRect,
    regions: Vec<OccupiedRegion>,
}
