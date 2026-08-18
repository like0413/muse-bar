/// 应用级共享状态及其只读访问接口。
pub mod state;

/// 配置插件并启动整个应用共享的 Tauri 运行时。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(state::AppState::new(env!("CARGO_PKG_VERSION")))
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
