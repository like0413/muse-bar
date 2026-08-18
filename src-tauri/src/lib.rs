use tauri::Manager;

/// 前端可调用的 Tauri 命令。
pub mod commands;

/// 用户设置的数据结构与默认值。
pub mod settings;

/// 应用级共享状态及其只读访问接口。
pub mod state;

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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::runtime::get_runtime_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
