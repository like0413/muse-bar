use tauri::Manager;

/// Child 窗口样式的阶段性技术验证。
#[cfg(target_os = "windows")]
pub mod child_window_test;

/// 前端可调用的 Tauri 命令。
pub mod commands;

/// Explorer 生命周期与任务栏重建消息监听。
#[cfg(target_os = "windows")]
pub mod explorer_monitor;

/// 与操作系统交互的条件编译边界。
pub mod platform;

/// 用户设置的数据结构与默认值。
pub mod settings;

/// 应用级共享状态及其只读访问接口。
pub mod state;

/// Windows 任务栏的发现与运行时信息。
#[cfg(target_os = "windows")]
pub mod taskbar;

/// 配置插件并启动整个应用共享的 Tauri 运行时。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let settings = settings::AppSettings::load(app.handle())?;
            app.manage(state::AppState::new(env!("CARGO_PKG_VERSION"), settings));

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            #[cfg(target_os = "windows")]
            {
                let bar_window = app
                    .get_webview_window("bar")
                    .ok_or_else(|| std::io::Error::other("无法找到 Bar 测试窗口"))?;
                let style_snapshot = child_window_test::apply_child_style(&bar_window)
                    .map_err(std::io::Error::other)?;
                log::info!(
                    "Bar Child 样式验证：修改前=0x{:08X}，目标=0x{:08X}，修改后=0x{:08X}",
                    style_snapshot.before,
                    style_snapshot.requested,
                    style_snapshot.applied
                );

                let taskbar = taskbar::find_main_taskbar().map_err(std::io::Error::other)?;
                let taskbar_rect =
                    taskbar::read_taskbar_rect(&taskbar).map_err(std::io::Error::other)?;
                let attachment =
                    child_window_test::attach_to_taskbar(&bar_window, &taskbar, &taskbar_rect)
                        .map_err(std::io::Error::other)?;
                log::info!(
                    "Bar Child 挂载验证：父窗口=0x{:X}，客户区位置=({}, {})，尺寸={}×{}",
                    attachment.parent,
                    attachment.client_x,
                    attachment.client_y,
                    attachment.width,
                    attachment.height
                );

                explorer_monitor::start(app.handle().clone())?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::diagnostics::get_taskbar_dpi,
            commands::diagnostics::get_taskbar_identity,
            commands::diagnostics::get_taskbar_rect,
            commands::runtime::get_runtime_info,
            commands::settings::get_settings,
            commands::settings::update_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
