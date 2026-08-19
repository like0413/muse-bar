use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

/// 保存整个应用进程唯一的 Windows 全局系统媒体管理器。
pub(crate) struct SystemMediaManager {
    manager: Option<GlobalSystemMediaTransportControlsSessionManager>,
}

impl SystemMediaManager {
    /// 请求 Windows 全局系统媒体管理器；失败时保留未初始化状态，避免阻止应用启动。
    pub(crate) fn initialize() -> Self {
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .and_then(|operation| operation.get())
            .map_err(|error| {
                log::error!("无法初始化 Windows 全局系统媒体管理器：{error}");
            })
            .ok();

        Self { manager }
    }

    /// 返回本次进程是否已经取得可供后续会话查询使用的管理器实例。
    pub(crate) fn is_initialized(&self) -> bool {
        self.manager.is_some()
    }
}
