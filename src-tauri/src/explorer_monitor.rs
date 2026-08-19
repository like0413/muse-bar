use std::{
    io,
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc::{self, Receiver, Sender},
        OnceLock,
    },
    thread,
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::{
    platform::windows::{
        w, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetModuleHandleW,
        PostQuitMessage, RegisterClassW, RegisterWindowMessageW, TranslateMessage, HINSTANCE, HWND,
        LPARAM, LRESULT, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_DESTROY, WNDCLASSW, WPARAM,
    },
    taskbar,
};

const TASKBAR_CREATED_EVENT: &str = "taskbar-created";
const RECOVERY_ATTEMPTS: usize = 30;
const RECOVERY_INTERVAL: Duration = Duration::from_millis(100);

static TASKBAR_CREATED_MESSAGE: AtomicU32 = AtomicU32::new(0);
static TASKBAR_CREATED_SENDER: OnceLock<Sender<()>> = OnceLock::new();

/// Explorer 重建任务栏后向所有前端窗口广播的新身份。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskbarCreatedEvent {
    hwnd: u64,
    explorer_process_id: u32,
}

/// 启动 Win32 消息线程和任务栏恢复工作线程。
pub fn start(app: AppHandle) -> io::Result<()> {
    let (sender, receiver) = mpsc::channel();
    TASKBAR_CREATED_SENDER
        .set(sender)
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

    Ok(())
}

/// 等待 TaskbarCreated 信号，并在 Explorer 就绪后广播新任务栏身份。
fn run_recovery_worker(app: AppHandle, receiver: Receiver<()>) {
    for () in receiver {
        match wait_for_recreated_taskbar() {
            Ok(taskbar) => {
                let event = TaskbarCreatedEvent {
                    hwnd: taskbar.handle_value(),
                    explorer_process_id: taskbar.explorer_process_id(),
                };

                if let Err(error) = app.emit(TASKBAR_CREATED_EVENT, event) {
                    log::error!("无法广播任务栏重建事件：{error}");
                }
            }
            Err(error) => log::error!("Explorer 重启后无法恢复任务栏身份：{error}"),
        }
    }
}

/// 在 Explorer 广播后短暂重试，等待新的 Shell_TrayWnd 完成创建。
fn wait_for_recreated_taskbar() -> Result<taskbar::TaskbarIdentity, String> {
    let mut last_error = "尚未尝试查找任务栏".to_owned();

    for attempt in 0..RECOVERY_ATTEMPTS {
        match taskbar::find_main_taskbar() {
            Ok(taskbar) => return Ok(taskbar),
            Err(error) => last_error = error,
        }

        if attempt + 1 < RECOVERY_ATTEMPTS {
            thread::sleep(RECOVERY_INTERVAL);
        }
    }

    Err(last_error)
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
            let _ = sender.send(());
        }
        return LRESULT(0);
    }

    if message == WM_DESTROY {
        unsafe { PostQuitMessage(0) };
        return LRESULT(0);
    }

    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}
