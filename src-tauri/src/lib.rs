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

/// 当前媒体应用的 WASAPI 回环采集与实时频谱分析。
mod spectrum;

/// 应用级共享状态及其只读访问接口。
mod state;

/// Windows 任务栏发现、占用布局、Child 宿主与 Explorer 恢复。
mod taskbar;

/// GitHub Release 更新检查、下载与安装状态。
mod updater;

/// 当前媒体应用的 Windows Core Audio 会话音量。
mod volume;

/// 音量按钮上方的无焦点独立浮层窗口。
mod volume_flyout;

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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let settings = settings::AppSettings::load(app.handle())?;
            let launch_on_startup = settings.launch_on_startup;
            let application_version = app.package_info().version.to_string();
            app.manage(state::AppState::new(application_version.clone(), settings));
            app.manage(updater::UpdateManager::new(application_version));

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
            app.manage(volume::ApplicationVolumeManager::start()?);
            app.manage(volume_flyout::VolumeFlyoutManager::default());
            app.manage(spectrum::SpectrumManager::default());

            if let Err(error) = autostart::synchronize(app.handle(), launch_on_startup) {
                log::error!("启动时无法同步开机启动设置：{error}");
            }

            app_lifecycle::create_tray(app)?;

            let explorer_monitor = taskbar::start_explorer_monitor(app.handle().clone())?;
            app.manage(explorer_monitor);

            updater::start_automatic_check(app.handle().clone());

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
            commands::spectrum::start_application_spectrum,
            commands::spectrum::stop_application_spectrum,
            commands::volume::get_current_application_volume,
            commands::volume::control_current_application_volume,
            commands::volume::show_application_volume_flyout,
            commands::volume::show_ready_application_volume_flyout,
            commands::volume::hide_application_volume_flyout,
            commands::runtime::get_runtime_info,
            commands::updater::get_update_status,
            commands::updater::check_for_update,
            commands::updater::install_update,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::taskbar::get_taskbar_monitors
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            app.state::<media::SystemMediaManager>().request_shutdown();
            app.state::<volume::ApplicationVolumeManager>()
                .request_shutdown();
            app.state::<spectrum::SpectrumManager>().request_shutdown();
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
