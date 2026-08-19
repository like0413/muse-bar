use tauri::Manager;

/// 任务栏 Child 窗口的样式、挂载与位置维护。
mod child_host;

/// 前端可调用的 Tauri 命令。
mod commands;

/// Explorer 生命周期与任务栏重建消息监听。
mod explorer_monitor;

/// 所有媒体会话的有效活动时间跟踪。
mod media_activity;

/// 当前选中媒体会话的播放与切歌控制。
mod media_control;

/// 与操作系统交互的条件编译边界。
mod platform;

/// 全局系统媒体管理器的进程级生命周期。
mod system_media;

/// 用户设置的数据结构与默认值。
mod settings;

/// 应用级共享状态及其只读访问接口。
mod state;

/// Windows 任务栏的发现与运行时信息。
mod taskbar;

/// Windows 11 任务栏原生控件占用区域的读取与回退。
mod taskbar_occupancy;

/// 配置插件并启动整个应用共享的 Tauri 运行时。
pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let settings = settings::AppSettings::load(app.handle())?;
            app.manage(state::AppState::new(env!("CARGO_PKG_VERSION"), settings));

            #[cfg(debug_assertions)]
            {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // 媒体管理器初始化期间也可能发生 WinRT 错误，因此调试日志必须先就绪。
            app.manage(system_media::SystemMediaManager::initialize(app.handle()));

            explorer_monitor::start(app.handle().clone())?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bar_layout::report_bar_content_width,
            commands::diagnostics::get_taskbar_dpi,
            commands::diagnostics::get_taskbar_identity,
            commands::diagnostics::get_taskbar_occupied_regions,
            commands::diagnostics::get_taskbar_rect,
            commands::media::get_current_media_metadata,
            commands::media::get_current_media_snapshot,
            commands::media::get_current_playback_capabilities,
            commands::media::get_current_playback_status,
            commands::media::get_current_timeline,
            commands::media::control_media,
            commands::media::get_media_session_source_app_ids,
            commands::media::get_media_session_identities,
            commands::media::get_media_session_activities,
            commands::media::is_system_media_manager_initialized,
            commands::media::refresh_selected_media_session,
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
