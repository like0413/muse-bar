use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindowBuilder,
};

use crate::platform::windows::{GetDpiForWindow, GetWindowRect, RECT};

const BAR_WINDOW_LABEL: &str = "bar";
pub(crate) const VOLUME_WINDOW_LABEL: &str = "volume";
const FLYOUT_WIDTH: f64 = 48.0;
const FLYOUT_HEIGHT: f64 = 176.0;
const FLYOUT_GAP: f64 = 4.0;
const FLYOUT_SHOWN_EVENT: &str = "application-volume-flyout-shown";
const FLYOUT_HIDDEN_EVENT: &str = "application-volume-flyout-hidden";

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VolumeFlyoutAnchor {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone)]
struct VolumeFlyoutRequest {
    anchor: VolumeFlyoutAnchor,
    session_key: u64,
    accent_color: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct VolumeFlyoutShownPayload {
    session_key: u64,
    accent_color: String,
}

#[derive(Default)]
pub(crate) struct VolumeFlyoutManager {
    ready: AtomicBool,
    pending: Mutex<Option<VolumeFlyoutRequest>>,
}

impl VolumeFlyoutManager {
    pub(crate) fn show<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        anchor: VolumeFlyoutAnchor,
        session_key: u64,
        accent_color: String,
    ) -> Result<(), String> {
        validate_anchor(anchor)?;
        validate_accent_color(&accent_color)?;
        *self.pending.lock().map_err(|_| "音量浮层状态已损坏")? = Some(VolumeFlyoutRequest {
            anchor,
            session_key,
            accent_color,
        });

        if app.get_webview_window(VOLUME_WINDOW_LABEL).is_none() {
            self.ready.store(false, Ordering::Release);
            let config = app
                .config()
                .app
                .windows
                .iter()
                .find(|config| config.label == VOLUME_WINDOW_LABEL)
                .ok_or_else(|| "配置中缺少音量浮层窗口".to_owned())?;
            WebviewWindowBuilder::from_config(app, config)
                .map_err(|error| format!("无法读取音量浮层配置：{error}"))?
                .build()
                .map_err(|error| format!("无法创建音量浮层：{error}"))?;
            return Ok(());
        }

        if self.ready.load(Ordering::Acquire) {
            self.apply_pending(app)?;
        }
        Ok(())
    }

    pub(crate) fn mark_ready_and_show<R: Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        self.ready.store(true, Ordering::Release);
        self.apply_pending(app)
    }

    pub(crate) fn hide<R: Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        *self.pending.lock().map_err(|_| "音量浮层状态已损坏")? = None;
        if let Some(window) = app.get_webview_window(VOLUME_WINDOW_LABEL) {
            window
                .hide()
                .map_err(|error| format!("无法隐藏音量浮层：{error}"))?;
            app.emit_to(VOLUME_WINDOW_LABEL, FLYOUT_HIDDEN_EVENT, ())
                .map_err(|error| format!("无法通知音量浮层隐藏：{error}"))?;
            app.emit_to(BAR_WINDOW_LABEL, FLYOUT_HIDDEN_EVENT, ())
                .map_err(|error| format!("无法通知 Bar 音量浮层已隐藏：{error}"))?;
        }
        Ok(())
    }

    fn apply_pending<R: Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        let request = self
            .pending
            .lock()
            .map_err(|_| "音量浮层状态已损坏")?
            .clone();
        let Some(request) = request else {
            return Ok(());
        };
        let bar = app
            .get_window(BAR_WINDOW_LABEL)
            .ok_or_else(|| "Bar 窗口当前不可用".to_owned())?;
        let flyout = app
            .get_webview_window(VOLUME_WINDOW_LABEL)
            .ok_or_else(|| "音量浮层窗口当前不可用".to_owned())?;
        let bar_handle = bar
            .hwnd()
            .map_err(|error| format!("无法读取 Bar 窗口句柄：{error}"))?;
        let mut bar_rect = RECT::default();
        unsafe { GetWindowRect(bar_handle, &mut bar_rect) }
            .map_err(|error| format!("无法读取 Bar 屏幕位置：{error}"))?;
        let scale = f64::from(unsafe { GetDpiForWindow(bar_handle) }.max(96)) / 96.0;
        let flyout_width = (FLYOUT_WIDTH * scale).round() as u32;
        let flyout_height = (FLYOUT_HEIGHT * scale).round() as u32;
        let anchor_center =
            bar_rect.left as f64 + (request.anchor.x + request.anchor.width / 2.0) * scale;
        let mut x = (anchor_center - f64::from(flyout_width) / 2.0).round() as i32;
        let mut y = bar_rect.top - flyout_height as i32 - (FLYOUT_GAP * scale).round() as i32;

        if let Some(monitor) = bar
            .current_monitor()
            .map_err(|error| format!("无法读取 Bar 所在显示器：{error}"))?
        {
            let monitor_position = monitor.position();
            let monitor_size = monitor.size();
            let monitor_right = monitor_position.x + monitor_size.width as i32;
            let monitor_bottom = monitor_position.y + monitor_size.height as i32;
            x = x.clamp(
                monitor_position.x,
                (monitor_right - flyout_width as i32).max(monitor_position.x),
            );
            if y < monitor_position.y {
                y = (bar_rect.bottom + (FLYOUT_GAP * scale).round() as i32)
                    .min(monitor_bottom - flyout_height as i32);
            }
        }

        flyout
            .set_size(PhysicalSize::new(flyout_width, flyout_height))
            .map_err(|error| format!("无法设置音量浮层尺寸：{error}"))?;
        flyout
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|error| format!("无法定位音量浮层：{error}"))?;
        flyout
            .emit(
                FLYOUT_SHOWN_EVENT,
                VolumeFlyoutShownPayload {
                    session_key: request.session_key,
                    accent_color: request.accent_color,
                },
            )
            .map_err(|error| format!("无法更新音量浮层会话：{error}"))?;
        flyout
            .show()
            .map_err(|error| format!("无法显示音量浮层：{error}"))
    }
}

fn validate_accent_color(color: &str) -> Result<(), String> {
    if color.len() == 7
        && color.starts_with('#')
        && color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        Ok(())
    } else {
        Err("音量浮层强调色无效".to_owned())
    }
}

fn validate_anchor(anchor: VolumeFlyoutAnchor) -> Result<(), String> {
    if [anchor.x, anchor.y, anchor.width, anchor.height]
        .into_iter()
        .all(f64::is_finite)
        && anchor.width > 0.0
        && anchor.height > 0.0
    {
        Ok(())
    } else {
        Err("音量按钮位置无效".to_owned())
    }
}

pub(crate) fn hide<R: Runtime>(app: &AppHandle<R>) {
    if let Some(manager) = app.try_state::<VolumeFlyoutManager>() {
        if let Err(error) = manager.hide(app) {
            log::error!("无法隐藏音量浮层：{error}");
        }
    }
}
