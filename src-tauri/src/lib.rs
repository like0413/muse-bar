use tauri::Manager;

/// 任务栏 Child 窗口的样式、挂载与位置维护。
#[cfg(target_os = "windows")]
mod child_host;

/// 前端可调用的 Tauri 命令。
mod commands;

/// Explorer 生命周期与任务栏重建消息监听。
#[cfg(target_os = "windows")]
mod explorer_monitor;

/// 与操作系统交互的条件编译边界。
mod platform;

/// 全局系统媒体管理器的进程级生命周期。
#[cfg(target_os = "windows")]
mod system_media;

/// 用户设置的数据结构与默认值。
mod settings;

/// 应用级共享状态及其只读访问接口。
mod state;

/// Windows 任务栏的发现与运行时信息。
#[cfg(target_os = "windows")]
mod taskbar;

/// 配置插件并启动整个应用共享的 Tauri 运行时。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let settings = settings::AppSettings::load(app.handle())?;
            app.manage(state::AppState::new(env!("CARGO_PKG_VERSION"), settings));

            #[cfg(target_os = "windows")]
            app.manage(system_media::SystemMediaManager::initialize(app.handle()));

            #[cfg(debug_assertions)]
            {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            #[cfg(target_os = "windows")]
            {
                explorer_monitor::start(app.handle().clone())?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::diagnostics::get_taskbar_dpi,
            commands::diagnostics::get_taskbar_identity,
            commands::diagnostics::get_taskbar_rect,
            commands::media::get_current_media_metadata,
            commands::media::get_current_playback_status,
            commands::media::get_media_session_source_app_ids,
            commands::media::is_system_media_manager_initialized,
            commands::runtime::get_runtime_info,
            commands::settings::get_settings,
            commands::settings::update_settings
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| {
        if let tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            // Explorer 重启会销毁其所有 Child。阻止“最后一个窗口消失”触发自然退出，
            // 让后台监听器有机会重建 Bar；带退出码的程序化退出仍会正常执行。
            api.prevent_exit();
        }
    });
}
