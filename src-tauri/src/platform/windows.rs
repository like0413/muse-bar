/// Windows API 使用的宽字符串指针与编译期宽字符串宏。
pub use ::windows::core::{w, PWSTR};

/// 原生句柄、窗口句柄与矩形类型，以及句柄释放函数。
pub use ::windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, RECT};

/// 查询进程可执行文件所需的最小权限、格式和 API。
pub use ::windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// 按指定窗口读取 DPI，避免错误使用全局系统 DPI。
pub use ::windows::Win32::UI::HiDpi::GetDpiForWindow;

/// 后续挂载任务栏窗口需要读取和修改的窗口样式 API。
pub use ::windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowLongPtrW, GetWindowThreadProcessId, SetWindowLongPtrW, GWL_EXSTYLE,
    GWL_STYLE, WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_POPUP,
};
