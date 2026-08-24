/// Windows API 使用的宽字符串指针与编译期宽字符串宏。
pub use ::windows::core::{w, BOOL, PWSTR};

/// 原生句柄、窗口句柄与矩形类型，以及句柄释放函数。
pub use ::windows::Win32::Foundation::{
    CloseHandle, COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
};

/// 读取 Windows 桌面窗口管理器当前强调色的 API。
pub use ::windows::Win32::Graphics::Dwm::DwmGetColorizationColor;

/// 屏幕坐标转换以及窗口所属显示器查询 API。
pub use ::windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, ScreenToClient, MONITORINFO, MONITOR_DEFAULTTONULL,
};

/// 获取创建隐藏消息窗口所需的当前模块句柄。
pub use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;

/// UI Automation 线程所需的 COM 初始化和对象创建 API。
pub use ::windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};

/// 查询进程可执行文件所需的最小权限、格式和 API。
pub use ::windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

/// 按指定窗口读取 DPI，避免错误使用全局系统 DPI。
pub use ::windows::Win32::UI::HiDpi::GetDpiForWindow;

/// 读取 Windows 11 任务栏可访问性树所需的 UI Automation 类型。
pub use ::windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, TreeScope_Descendants,
};

/// 后续挂载任务栏窗口需要读取和修改的窗口样式 API。
pub use ::windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, EnumChildWindows, FindWindowW,
    GetClassNameW, GetMessageW, GetParent, GetWindowLongPtrW, GetWindowRect,
    GetWindowThreadProcessId, IsWindow, IsWindowVisible, PostQuitMessage, RegisterClassW,
    RegisterWindowMessageW, SetLayeredWindowAttributes, SetParent, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, TranslateMessage, GWL_EXSTYLE, GWL_STYLE, HWND_TOP, LWA_ALPHA,
    MONITORINFOF_PRIMARY, MSG, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW,
    SW_HIDE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_DESTROY, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN,
    WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_POPUP,
};
