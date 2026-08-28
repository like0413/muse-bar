use tauri::{AppHandle, Runtime};
use tauri_plugin_autostart::ManagerExt;

/// 让 Windows 启动项与持久化设置保持一致。
pub(crate) fn synchronize<R: Runtime>(
    app: &AppHandle<R>,
    should_launch_on_startup: bool,
) -> Result<(), String> {
    let manager = app.autolaunch();
    let is_enabled = manager
        .is_enabled()
        .map_err(|error| format!("无法读取开机启动状态：{error}"))?;
    if is_enabled == should_launch_on_startup {
        return Ok(());
    }

    if should_launch_on_startup {
        manager
            .enable()
            .map_err(|error| format!("无法注册开机启动：{error}"))
    } else {
        manager
            .disable()
            .map_err(|error| format!("无法移除开机启动：{error}"))
    }
}
