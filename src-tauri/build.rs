/// 生成 Tauri 编译期配置和平台资源。
fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "control_media",
            "control_current_application_volume",
            "show_application_volume_flyout",
            "show_ready_application_volume_flyout",
            "hide_application_volume_flyout",
            "get_current_application_volume",
            "get_current_media_snapshot",
            "get_media_session_activities",
            "get_media_session_identities",
            "get_runtime_info",
            "get_settings",
            "get_taskbar_dpi",
            "get_taskbar_identity",
            "get_taskbar_monitors",
            "get_taskbar_occupied_regions",
            "get_update_status",
            "get_windows_version",
            "install_update",
            "open_log_directory",
            "open_settings_window",
            "refresh_selected_media_session",
            "report_bar_content_width",
            "set_bar_media_available",
            "show_ready_settings_window",
            "update_settings",
            "check_for_update",
        ]),
    ))
    .expect("error while building Tauri application resources")
}
