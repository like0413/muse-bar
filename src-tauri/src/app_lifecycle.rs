use std::io;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::WebviewWindowBuilder,
    App, AppHandle, Manager, Runtime,
};

use crate::state::AppState;

pub(crate) const SETTINGS_WINDOW_LABEL: &str = "settings";
const BAR_WINDOW_LABEL: &str = "bar";
const TRAY_ID: &str = "muse-bar-tray";
const MENU_SETTINGS_ID: &str = "open-settings";
const MENU_TOGGLE_BAR_ID: &str = "toggle-bar";
const MENU_EXIT_ID: &str = "exit";

/// 创建进程级托盘图标以及设置、Bar 显隐和退出菜单。
pub(crate) fn create_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let settings_item = MenuItem::with_id(app, MENU_SETTINGS_ID, "设置", true, None::<&str>)?;
    let toggle_bar_item =
        MenuItem::with_id(app, MENU_TOGGLE_BAR_ID, "隐藏 Bar", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let exit_item = MenuItem::with_id(app, MENU_EXIT_ID, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&settings_item, &toggle_bar_item, &separator, &exit_item],
    )?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "应用配置中缺少托盘图标"))?;

    let toggle_bar_menu = toggle_bar_item.clone();
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Muse Bar")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            MENU_SETTINGS_ID => log_result("无法打开设置窗口", open_settings_window(app)),
            MENU_TOGGLE_BAR_ID => match toggle_bar(app) {
                Ok(is_visible) => {
                    let label = if is_visible {
                        "隐藏 Bar"
                    } else {
                        "显示 Bar"
                    };
                    if let Err(error) = toggle_bar_menu.set_text(label) {
                        log::error!("无法更新 Bar 显隐菜单文字：{error}");
                    }
                }
                Err(error) => log::error!("无法切换 Bar 显示状态：{error}"),
            },
            MENU_EXIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                log_result("无法打开设置窗口", open_settings_window(tray.app_handle()));
            }
        })
        .build(app)?;

    Ok(())
}

/// 聚焦已经显示的设置窗口；尚未创建时先创建不可见窗口等待前端就绪。
pub(crate) fn open_settings_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
        let is_visible = window
            .is_visible()
            .map_err(|error| format!("无法读取设置窗口可见性：{error}"))?;
        if is_visible {
            window
                .unminimize()
                .map_err(|error| format!("无法还原设置窗口：{error}"))?;
            window
                .set_focus()
                .map_err(|error| format!("无法聚焦设置窗口：{error}"))?;
        }
        return Ok(());
    }

    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == SETTINGS_WINDOW_LABEL)
        .ok_or_else(|| "配置中缺少设置窗口".to_owned())?;
    WebviewWindowBuilder::from_config(app, config)
        .map_err(|error| format!("无法读取设置窗口配置：{error}"))?
        .build()
        .map_err(|error| format!("无法创建设置窗口：{error}"))?;

    Ok(())
}

/// 在设置前端完成数据读取和首次渲染后显示并聚焦窗口。
pub(crate) fn show_ready_settings_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = app
        .get_webview_window(SETTINGS_WINDOW_LABEL)
        .ok_or_else(|| "准备显示时设置窗口已经销毁".to_owned())?;
    window
        .show()
        .map_err(|error| format!("无法显示设置窗口：{error}"))?;
    window
        .set_focus()
        .map_err(|error| format!("无法聚焦设置窗口：{error}"))
}

/// 切换本次运行中的 Bar 显示状态；该状态不会写入用户设置。
fn toggle_bar<R: Runtime>(app: &AppHandle<R>) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let is_enabled = state.toggle_bar_enabled_by_user();
    let operation_result = synchronize_bar_visibility(app);

    if let Err(error) = operation_result {
        // 原生窗口操作失败时恢复状态，避免菜单状态与实际可见性永久相反。
        state.toggle_bar_enabled_by_user();
        return Err(error);
    }

    Ok(is_enabled)
}

/// 根据用户选择和媒体可用状态，将 Bar 原生窗口同步到最终显隐结果。
pub(crate) fn synchronize_bar_visibility<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let should_show = app.state::<AppState>().should_show_bar();
    if should_show {
        // Child 窗口隐藏后不能只调用 Tauri 的 show：那样不会重新校准窗口样式、
        // 父子关系和任务栏客户区位置，WebView 看似恢复却可能失去鼠标命中。
        // 统一复用 Explorer 恢复通道，由 Child 宿主完成挂载并原生显示窗口。
        crate::taskbar::request_recovery()?;
    } else if let Some(window) = app.get_window(BAR_WINDOW_LABEL) {
        crate::taskbar::hide_window(&window)?;
    }

    Ok(())
}

/// 为托盘回调统一记录失败，不让一次菜单操作终止应用。
fn log_result(context: &str, result: Result<(), String>) {
    if let Err(error) = result {
        log::error!("{context}：{error}");
    }
}
