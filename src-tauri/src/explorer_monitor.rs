use std::{
    io,
    sync::{
        atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
        Arc, Mutex, OnceLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use tauri::{
    webview::WebviewBuilder, window::WindowBuilder, AppHandle, Manager, PhysicalPosition,
    PhysicalSize,
};

use crate::{
    background_worker::{join_with_timeout, WORKER_SHUTDOWN_TIMEOUT},
    child_host,
    platform::windows::{
        w, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetModuleHandleW,
        PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW, TranslateMessage,
        HINSTANCE, HWND, LPARAM, LRESULT, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_DESTROY,
        WNDCLASSW, WPARAM,
    },
    settings::TaskbarPosition,
    state::AppState,
    taskbar, taskbar_occupancy,
};

const RECOVERY_ATTEMPTS: usize = 30;
const RECOVERY_INTERVAL: Duration = Duration::from_millis(100);
const TASKBAR_LAYOUT_POLL_INTERVAL: Duration = Duration::from_millis(500);
const MISSING_BAR_RECOVERY_INTERVAL: Duration = Duration::from_secs(2);
const BAR_WINDOW_LABEL: &str = "bar";

static TASKBAR_CREATED_MESSAGE: AtomicU32 = AtomicU32::new(0);
static TASKBAR_CREATED_SENDER: OnceLock<SyncSender<()>> = OnceLock::new();
static EXPLORER_MONITOR_SHUTDOWN: OnceLock<Arc<AtomicBool>> = OnceLock::new();

/// 管理三个 Explorer 后台线程的停止信号和退出等待。
pub(crate) struct ExplorerMonitor {
    shutdown: Arc<AtomicBool>,
    message_window: Arc<AtomicIsize>,
    workers: Mutex<Vec<JoinHandle<()>>>,
}

impl ExplorerMonitor {
    /// 通知全部线程停止；实际等待在线程状态析构时完成。
    pub(crate) fn request_shutdown(&self) {
        if self.shutdown.swap(true, Ordering::AcqRel) {
            return;
        }

        taskbar_occupancy::invalidate_positioning_regions_cache();
        let _ = TASKBAR_CREATED_SENDER
            .get()
            .and_then(|sender| sender.try_send(()).ok());
        let window = self.message_window.load(Ordering::Acquire);
        if window != 0 {
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(window as *mut _)),
                    WM_CLOSE,
                    WPARAM::default(),
                    LPARAM::default(),
                )
            };
        }
    }
}

impl Drop for ExplorerMonitor {
    fn drop(&mut self) {
        self.request_shutdown();
        if let Ok(workers) = self.workers.get_mut() {
            for worker in std::mem::take(workers) {
                join_with_timeout(worker, "Explorer", WORKER_SHUTDOWN_TIMEOUT);
            }
        }
    }
}

/// 会影响歌词 Bar 可用区域的最小任务栏布局快照。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LyricsTaskbarLayout {
    taskbar_handle: u64,
    taskbar_dpi: u32,
    position: TaskbarPosition,
    manual_offset: i32,
    span_left: i32,
    span_width: i32,
}

/// 启动 Win32 消息、任务栏恢复和歌词可用区域监控线程。
pub fn start(app: AppHandle) -> io::Result<ExplorerMonitor> {
    // Explorer 在短时间内重复广播时，只保留一个待处理的恢复任务。
    // 容量为 1 的同步通道既能合并重复通知，也能防止任务无限积压。
    let (sender, receiver) = mpsc::sync_channel(1);
    TASKBAR_CREATED_SENDER
        .set(sender.clone())
        .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "Explorer 监听器已经启动"))?;
    let shutdown = Arc::new(AtomicBool::new(false));
    EXPLORER_MONITOR_SHUTDOWN
        .set(Arc::clone(&shutdown))
        .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "Explorer 停止状态已经创建"))?;
    let message_window = Arc::new(AtomicIsize::new(0));
    let recovery_app = app.clone();
    let recovery_shutdown = Arc::clone(&shutdown);
    let recovery_worker = thread::Builder::new()
        .name("muse-bar-explorer-recovery".to_owned())
        .spawn(move || run_recovery_worker(recovery_app, receiver, recovery_shutdown))?;

    let layout_shutdown = Arc::clone(&shutdown);
    let layout_worker = thread::Builder::new()
        .name("muse-bar-taskbar-layout".to_owned())
        .spawn(move || run_taskbar_layout_monitor(app, layout_shutdown));
    let layout_worker = match layout_worker {
        Ok(worker) => worker,
        Err(error) => {
            shutdown.store(true, Ordering::Release);
            let _ = recovery_worker.join();
            return Err(error);
        }
    };

    let message_shutdown = Arc::clone(&shutdown);
    let message_window_handle = Arc::clone(&message_window);
    let message_worker = thread::Builder::new()
        .name("muse-bar-windows-messages".to_owned())
        .spawn(move || {
            if let Err(error) = run_message_window(message_shutdown, message_window_handle) {
                log::error!("Explorer 消息监听器已停止：{error}");
            }
        });
    let message_worker = match message_worker {
        Ok(worker) => worker,
        Err(error) => {
            shutdown.store(true, Ordering::Release);
            let _ = recovery_worker.join();
            let _ = layout_worker.join();
            return Err(error);
        }
    };

    // 配置中的 Bar 只作为窗口模板。通过同一条恢复通道进行首次创建，可以保证
    // 原生宿主先挂到任务栏，再创建真正的 Child WebView。
    sender
        .try_send(())
        .map_err(|_| io::Error::other("无法提交 Bar 首次创建任务"))?;

    Ok(ExplorerMonitor {
        shutdown,
        message_window,
        workers: Mutex::new(vec![recovery_worker, layout_worker, message_worker]),
    })
}

/// 请求恢复工作线程重新创建当前缺失的 Bar。
pub(crate) fn request_recovery() -> Result<(), String> {
    if EXPLORER_MONITOR_SHUTDOWN
        .get()
        .is_some_and(|shutdown| shutdown.load(Ordering::Acquire))
    {
        return Err("Explorer 恢复监听器正在关闭".to_owned());
    }
    TASKBAR_CREATED_SENDER
        .get()
        .ok_or_else(|| "Explorer 恢复监听器尚未启动".to_owned())?
        .try_send(())
        .or_else(|error| match error {
            mpsc::TrySendError::Full(_) => Ok(()),
            mpsc::TrySendError::Disconnected(_) => Err("Explorer 恢复工作线程已经停止".to_owned()),
        })
}

/// 等待 TaskbarCreated 信号并恢复 Bar。
fn run_recovery_worker(app: AppHandle, receiver: Receiver<()>, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Acquire) {
        match receiver.recv_timeout(RECOVERY_INTERVAL) {
            Ok(()) => {
                taskbar_occupancy::invalidate_positioning_regions_cache();
                if let Err(error) = wait_for_bar_recovery(&app, &shutdown) {
                    if !shutdown.load(Ordering::Acquire) {
                        log::error!("Explorer 重启后无法恢复 Bar：{error}");
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// 定期读取 Explorer XAML 任务栏布局，仅在稳定的目标范围变化后调整歌词 Bar。
fn run_taskbar_layout_monitor(app: AppHandle, shutdown: Arc<AtomicBool>) {
    let mut previous_layout = None;
    let mut pending_layout = None;
    let mut last_recovery_request = None;
    while !shutdown.load(Ordering::Acquire) {
        thread::sleep(TASKBAR_LAYOUT_POLL_INTERVAL);
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        if bar_requires_recovery(&app).unwrap_or(true)
            && recovery_request_is_due(last_recovery_request, Instant::now())
        {
            let _ = request_recovery();
            last_recovery_request = Some(Instant::now());
        }
        if synchronize_lyrics_taskbar_layout(&app, &mut previous_layout, &mut pending_layout)
            .is_err()
        {
            // Explorer 重启和 Bar 重建期间读取失败属于正常过渡；清空快照即可在恢复后重试。
            previous_layout = None;
            pending_layout = None;
        }
    }
}

/// 判断 Bar 是否缺失、句柄已失效或已经脱离当前目标任务栏。
fn bar_requires_recovery(app: &AppHandle) -> Result<bool, String> {
    let Some(bar_window) = app.get_window(BAR_WINDOW_LABEL) else {
        return Ok(true);
    };
    if !child_host::is_window_alive(&bar_window) {
        return Ok(true);
    }
    let settings = app.state::<AppState>().settings()?;
    let taskbar = taskbar::find_taskbar(&settings.target_monitor)?;
    Ok(!child_host::is_attached_to_taskbar(&bar_window, &taskbar))
}

fn recovery_request_is_due(last_request: Option<Instant>, now: Instant) -> bool {
    last_request.map_or(true, |last_request| {
        now.saturating_duration_since(last_request) >= MISSING_BAR_RECOVERY_INTERVAL
    })
}

/// 连续两次读取到相同的任务栏可用区域后，再复用现有宽度动画。
fn synchronize_lyrics_taskbar_layout(
    app: &AppHandle,
    previous_layout: &mut Option<LyricsTaskbarLayout>,
    pending_layout: &mut Option<LyricsTaskbarLayout>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = state.settings()?;
    if !settings.lyrics_enabled {
        *previous_layout = None;
        *pending_layout = None;
        return Ok(());
    }

    let bar_window = app
        .get_window(BAR_WINDOW_LABEL)
        .ok_or_else(|| "Bar 原生宿主尚未创建".to_owned())?;
    let bar_webview = app
        .get_webview(BAR_WINDOW_LABEL)
        .ok_or_else(|| "Bar WebView 尚未创建".to_owned())?;
    let taskbar = taskbar::find_taskbar(&settings.target_monitor)?;
    if !child_host::is_attached_to_taskbar(&bar_window, &taskbar) {
        return Err("Bar 尚未挂载到目标任务栏".to_owned());
    }

    let taskbar_rect = taskbar::read_taskbar_rect(&taskbar)?;
    let taskbar_dpi = taskbar::read_taskbar_dpi(&taskbar)?;
    let positioning_regions = taskbar_occupancy::read_positioning_regions(&taskbar, &taskbar_rect);
    let positioning_span = taskbar_occupancy::resolve_available_span(
        settings.position,
        &taskbar_rect,
        &positioning_regions,
    );
    let current_layout = LyricsTaskbarLayout {
        taskbar_handle: taskbar.handle_value(),
        taskbar_dpi: taskbar_dpi.dpi(),
        position: settings.position,
        manual_offset: settings.manual_offset,
        span_left: positioning_span.left(),
        span_width: positioning_span.width(),
    };
    if previous_layout.as_ref() == Some(&current_layout) {
        *pending_layout = None;
        return Ok(());
    }
    if pending_layout.as_ref() != Some(&current_layout) {
        // Windows 11 会先更新部分 XAML 元素再完成整组居中。保留候选快照，
        // 下一轮仍相同才说明它不是动画中间帧。
        *pending_layout = Some(current_layout);
        return Ok(());
    }

    let logical_width = taskbar_dpi
        .physical_to_logical(positioning_span.width())
        .round()
        .max(1.0) as u32;
    state.report_bar_content_width(1.0, Some(logical_width))?;
    let (animation_revision, latest_animation_revision) = state.begin_bar_width_animation();
    child_host::animate_window_width(child_host::WindowWidthAnimationRequest {
        bar_window,
        bar_webview,
        taskbar: &taskbar,
        taskbar_rect: &taskbar_rect,
        taskbar_dpi: &taskbar_dpi,
        position: settings.position,
        manual_offset: settings.manual_offset,
        preferred_screen_x: Some(positioning_span.left()),
        target_width: positioning_span.width(),
        animation_revision,
        latest_animation_revision,
    })?;
    *previous_layout = Some(current_layout);
    *pending_layout = None;

    Ok(())
}

/// 重试完整恢复流程，等待任务栏、Tauri 窗口注册表和 WebView2 都进入可用状态。
fn wait_for_bar_recovery(app: &AppHandle, shutdown: &AtomicBool) -> Result<(), String> {
    let mut last_error = "尚未尝试恢复 Bar".to_owned();

    for attempt in 0..RECOVERY_ATTEMPTS {
        if shutdown.load(Ordering::Acquire) {
            return Ok(());
        }
        match recover_bar_once(app) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }

        if attempt + 1 < RECOVERY_ATTEMPTS {
            thread::sleep(RECOVERY_INTERVAL);
        }
    }

    Err(last_error)
}

/// 执行一次 Bar 恢复：先创建并挂载原生宿主，再在宿主内创建 Child WebView。
///
/// Tauri 的 Windows 实现要求从非主线程调用窗口与 WebView 创建接口；接口内部会把
/// 真正的创建工作派发到事件循环，避免 WebView2 同步初始化造成主线程死锁。
fn recover_bar_once(app: &AppHandle) -> Result<(), String> {
    let settings = app.state::<AppState>().settings()?;
    let taskbar = taskbar::find_taskbar(&settings.target_monitor)?;
    let taskbar_rect = taskbar::read_taskbar_rect(&taskbar)?;
    let taskbar_dpi = taskbar::read_taskbar_dpi(&taskbar)?;

    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == BAR_WINDOW_LABEL)
        .ok_or_else(|| "配置中缺少 Bar 窗口".to_owned())?;

    let (bar_window, bar_recreated) = match app.get_window(BAR_WINDOW_LABEL) {
        Some(window) if child_host::is_window_alive(&window) => (window, false),
        Some(window) => {
            // Explorer 会销毁其 Child。Tauri 的窗口注册表可能稍晚一拍才移除旧标签，
            // 因此先请求清理并让外层重试，避免用同一标签创建出重复窗口。
            let _ = window.destroy();
            return Err("旧 Bar 已失效，正在等待 Tauri 清理窗口标签".to_owned());
        }
        None => {
            // Explorer 销毁原生 Child 后，旧 WebView 标签可能比窗口标签更晚移除。
            // 先关闭残留项并等待下一次重试，避免创建同名 WebView 失败。
            if let Some(webview) = app.get_webview(BAR_WINDOW_LABEL) {
                let _ = webview.close();
                return Err("旧 Bar WebView 尚未清理，正在等待标签释放".to_owned());
            }

            let window = WindowBuilder::from_config(app, config)
                .map_err(|error| format!("无法读取 Bar 窗口配置：{error}"))?
                .build()
                .map_err(|error| format!("无法创建 Bar 原生宿主：{error}"))?;
            (window, true)
        }
    };

    let should_show = app.state::<AppState>().should_show_bar();
    let host_size = match child_host::attach_window(
        &bar_window,
        &taskbar,
        &taskbar_rect,
        &taskbar_dpi,
        child_host::AttachWindowOptions {
            position: settings.position,
            manual_offset: settings.manual_offset,
            should_show,
        },
    ) {
        Ok(host_size) => host_size,
        Err(error) => {
            if bar_recreated {
                let _ = bar_window.destroy();
            }
            return Err(error);
        }
    };

    if bar_recreated {
        let webview_builder = WebviewBuilder::from_config(config);
        let webview_size = PhysicalSize::new(host_size.width, host_size.height);
        let bar_webview = bar_window
            .add_child(webview_builder, PhysicalPosition::new(0, 0), webview_size)
            .map_err(|error| {
                let _ = bar_window.destroy();
                format!("无法在 Bar 宿主内创建 Child WebView：{error}")
            })?;

        // 宽度动画只调整原生宿主，由 Tauri 按父窗口客户区同步 Child WebView，
        // 避免两个独立的尺寸消息队列产生短暂空隙或错误的自动缩放比例。
        if let Err(error) = bar_webview.set_auto_resize(true) {
            let _ = bar_window.destroy();
            return Err(format!("无法启用 Bar WebView 自动尺寸同步：{error}"));
        }
    } else {
        let bar_webview = app
            .get_webview(BAR_WINDOW_LABEL)
            .ok_or_else(|| "Bar 原生宿主存在，但 Child WebView 不存在".to_owned())?;
        bar_webview
            .set_auto_resize(true)
            .map_err(|error| format!("无法恢复 Bar WebView 自动尺寸同步：{error}"))?;
    }

    // 用户临时隐藏和无媒体状态都会跨 Explorer 重建保留，避免恢复流程错误显示 Bar。
    if !app.state::<AppState>().should_show_bar() {
        child_host::hide_window(&bar_window)?;
    }

    Ok(())
}

/// 创建不可见的顶层窗口并运行接收系统广播所需的消息循环。
fn run_message_window(
    shutdown: Arc<AtomicBool>,
    message_window: Arc<AtomicIsize>,
) -> Result<(), String> {
    let taskbar_created_message = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
    if taskbar_created_message == 0 {
        return Err("无法注册 TaskbarCreated 消息".to_owned());
    }
    TASKBAR_CREATED_MESSAGE.store(taskbar_created_message, Ordering::Release);
    let module = unsafe { GetModuleHandleW(None) }
        .map_err(|error| format!("无法获取当前模块句柄：{error}"))?;
    let instance = HINSTANCE(module.0);
    let window_class = WNDCLASSW {
        lpfnWndProc: Some(message_window_proc),
        hInstance: instance,
        lpszClassName: w!("MuseBarExplorerMessageWindow"),
        ..Default::default()
    };

    let class_atom = unsafe { RegisterClassW(&window_class) };
    if class_atom == 0 {
        return Err("无法注册 Explorer 消息窗口类".to_owned());
    }

    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("MuseBarExplorerMessageWindow"),
            w!(""),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            None,
        )
    }
    .map_err(|error| format!("无法创建 Explorer 消息窗口：{error}"))?;
    message_window.store(window.0 as isize, Ordering::Release);
    if shutdown.load(Ordering::Acquire) {
        let _ =
            unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM::default(), LPARAM::default()) };
    }
    let mut message = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if result.0 == -1 {
            return Err("Explorer 消息循环读取失败".to_owned());
        }
        if result.0 == 0 {
            break;
        }

        unsafe {
            let _ = TranslateMessage(&message);
            let _ = DispatchMessageW(&message);
        }
    }

    message_window.store(0, Ordering::Release);

    Ok(())
}

/// 接收原始 Windows 消息，并转交 Explorer 恢复通知。
unsafe extern "system" fn message_window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let taskbar_created_message = TASKBAR_CREATED_MESSAGE.load(Ordering::Acquire);
    if taskbar_created_message != 0 && message == taskbar_created_message {
        if let Some(sender) = TASKBAR_CREATED_SENDER.get() {
            // 窗口过程必须尽快返回，不能等待正在运行的恢复任务。
            // 队列已满表示已有一次恢复等待执行，此时无需重复提交。
            let _ = sender.try_send(());
        }
        return LRESULT(0);
    }

    if message == WM_DESTROY {
        unsafe { PostQuitMessage(0) };
        return LRESULT(0);
    }

    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}
