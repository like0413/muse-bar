use std::{
    io,
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc::{self, Receiver, SyncSender},
        OnceLock,
    },
    thread,
    time::Duration,
};

use tauri::{
    webview::WebviewBuilder, window::WindowBuilder, AppHandle, Manager, PhysicalPosition,
    PhysicalSize,
};

use crate::{
    child_host,
    platform::windows::{
        w, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetModuleHandleW,
        PostQuitMessage, RegisterClassW, RegisterWindowMessageW, TranslateMessage, HINSTANCE, HWND,
        LPARAM, LRESULT, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_DESTROY, WNDCLASSW, WPARAM,
    },
    state::AppState,
    taskbar,
};

const RECOVERY_ATTEMPTS: usize = 30;
const RECOVERY_INTERVAL: Duration = Duration::from_millis(100);
const BAR_WINDOW_LABEL: &str = "bar";

static TASKBAR_CREATED_MESSAGE: AtomicU32 = AtomicU32::new(0);
static TASKBAR_CREATED_SENDER: OnceLock<SyncSender<()>> = OnceLock::new();

/// 启动 Win32 消息线程和任务栏恢复工作线程。
pub fn start(app: AppHandle) -> io::Result<()> {
    // Explorer 在短时间内重复广播时，只保留一个待处理的恢复任务。
    // 容量为 1 的同步通道既能合并重复通知，也能防止任务无限积压。
    let (sender, receiver) = mpsc::sync_channel(1);
    TASKBAR_CREATED_SENDER
        .set(sender.clone())
        .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "Explorer 监听器已经启动"))?;

    thread::Builder::new()
        .name("muse-bar-explorer-recovery".to_owned())
        .spawn(move || run_recovery_worker(app, receiver))?;

    thread::Builder::new()
        .name("muse-bar-windows-messages".to_owned())
        .spawn(|| {
            if let Err(error) = run_message_window() {
                log::error!("Explorer 消息监听器已停止：{error}");
            }
        })?;

    // 配置中的 Bar 只作为窗口模板。通过同一条恢复通道进行首次创建，可以保证
    // 原生宿主先挂到任务栏，再创建真正的 Child WebView。
    sender
        .try_send(())
        .map_err(|_| io::Error::other("无法提交 Bar 首次创建任务"))?;

    Ok(())
}

/// 请求恢复工作线程重新创建当前缺失的 Bar。
pub(crate) fn request_recovery() -> Result<(), String> {
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
fn run_recovery_worker(app: AppHandle, receiver: Receiver<()>) {
    for () in receiver {
        if let Err(error) = wait_for_bar_recovery(&app) {
            log::error!("Explorer 重启后无法恢复 Bar：{error}");
        }
    }
}

/// 重试完整恢复流程，等待任务栏、Tauri 窗口注册表和 WebView2 都进入可用状态。
fn wait_for_bar_recovery(app: &AppHandle) -> Result<(), String> {
    let mut last_error = "尚未尝试恢复 Bar".to_owned();

    for attempt in 0..RECOVERY_ATTEMPTS {
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

    let host_size = match child_host::attach_window(
        &bar_window,
        &taskbar,
        &taskbar_rect,
        &taskbar_dpi,
        settings.position,
        settings.manual_offset,
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
fn run_message_window() -> Result<(), String> {
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

    let _window = unsafe {
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

    Ok(())
}

/// 接收原始 Windows 消息，并将 TaskbarCreated 快速转交给恢复线程。
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
