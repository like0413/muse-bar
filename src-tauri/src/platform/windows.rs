/// 原生窗口句柄与矩形类型。
pub use ::windows::Win32::Foundation::{HWND, RECT};

/// 按指定窗口读取 DPI，避免错误使用全局系统 DPI。
pub use ::windows::Win32::UI::HiDpi::GetDpiForWindow;

/// 后续挂载任务栏窗口需要读取和修改的窗口样式 API。
pub use ::windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, GWL_STYLE, WINDOW_EX_STYLE, WINDOW_STYLE,
    WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_POPUP,
};
