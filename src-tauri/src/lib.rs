use tauri::Manager;

/// 托盘、设置窗口与应用退出生命周期。
mod app_lifecycle;

/// Windows 当前用户开机启动项同步。
mod autostart;

/// 后台线程统一的有界退出等待。
mod background_worker;

/// 前端可调用的 Tauri 命令。
mod commands;

/// 系统媒体会话、选择、控制与展示数据。
mod media;

/// 与操作系统交互的条件编译边界。
mod platform;

/// 用户设置模型、持久化与更新事务。
mod settings;

/// 应用级共享状态及其只读访问接口。
mod state;

/// 进程启动关键路径的低开销一次性里程碑。
mod startup_metrics;

/// Windows 任务栏发现、占用布局、Child 宿主与 Explorer 恢复。
mod taskbar;

/// 配置插件并启动整个应用共享的 Tauri 运行时。
pub fn run() {
    startup_metrics::begin();
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
            app.manage(media::SystemMediaManager::initialize(app.handle()));

            if let Err(error) = autostart::synchronize(app.handle(), launch_on_startup) {
                log::error!("启动时无法同步开机启动设置：{error}");
            }

            app_lifecycle::create_tray(app)?;

            let explorer_monitor = taskbar::start_explorer_monitor(app.handle().clone())?;
            app.manage(explorer_monitor);

            startup_metrics::mark_setup_complete();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bar_layout::report_bar_content_width,
            commands::diagnostics::get_taskbar_dpi,
            commands::diagnostics::get_taskbar_identity,
            commands::diagnostics::get_taskbar_occupied_regions,
            commands::diagnostics::get_windows_version,
            commands::diagnostics::open_log_directory,
            commands::lifecycle::open_settings_window,
            commands::lifecycle::set_bar_media_available,
            commands::lifecycle::show_ready_settings_window,
            commands::media::get_current_media_snapshot,
            commands::media::control_media,
            commands::media::get_media_session_identities,
            commands::media::get_media_session_activities,
            commands::media::refresh_selected_media_session,
            commands::runtime::get_runtime_info,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::taskbar::get_taskbar_monitors
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            app.state::<media::SystemMediaManager>().request_shutdown();
            app.state::<taskbar::ExplorerMonitor>().request_shutdown();
        }
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
