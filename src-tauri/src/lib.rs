use tauri::Manager;

/// 托盘、设置窗口与应用退出生命周期。
mod app_lifecycle;

/// Windows 当前用户开机启动项同步。
mod autostart;

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
        // 单实例插件必须最先注册，确保第二个进程不会先创建托盘或后台监听器。
        .plugin(tauri_plugin_single_instance::init(
            |app, _arguments, _working_directory| {
                if let Err(error) = app_lifecycle::open_settings_window(app) {
                    log::error!("第二次启动时无法唤醒设置窗口：{error}");
                }
            },
        ))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("Muse Bar")
                .build(),
        )
        .setup(|app| {
            let settings = settings::AppSettings::load(app.handle())?;
            let launch_on_startup = settings.launch_on_startup;
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

            if let Err(error) = autostart::synchronize(app.handle(), launch_on_startup) {
                log::error!("启动时无法同步开机启动设置：{error}");
            }

            app_lifecycle::create_tray(app)?;

            explorer_monitor::start(app.handle().clone())?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bar_layout::report_bar_content_width,
            commands::diagnostics::get_taskbar_dpi,
            commands::diagnostics::get_taskbar_identity,
            commands::diagnostics::get_taskbar_occupied_regions,
            commands::diagnostics::get_taskbar_rect,
            commands::diagnostics::get_windows_version,
            commands::diagnostics::open_log_directory,
            commands::lifecycle::open_settings_window,
            commands::lifecycle::set_bar_media_available,
            commands::lifecycle::show_ready_settings_window,
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
            // 设置窗口可正常销毁；只有托盘“退出”的带退出码请求才真正结束进程。
            api.prevent_exit();
        }
    });
}
