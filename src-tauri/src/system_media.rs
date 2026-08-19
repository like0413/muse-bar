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

    /// 枚举当前系统媒体会话，并按 Windows 返回顺序提取 Source App ID。
    pub(crate) fn source_app_ids(&self) -> Result<Vec<String>, String> {
        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| "Windows 全局系统媒体管理器尚未初始化".to_owned())?;
        let sessions = manager
            .GetSessions()
            .map_err(|error| format!("无法枚举系统媒体会话：{error}"))?;
        let session_count = sessions
            .Size()
            .map_err(|error| format!("无法读取系统媒体会话数量：{error}"))?;
        let mut source_app_ids = Vec::with_capacity(session_count as usize);

        for index in 0..session_count {
            let session = sessions
                .GetAt(index)
                .map_err(|error| format!("无法读取第 {} 个系统媒体会话：{error}", index + 1))?;
            let source_app_id = session.SourceAppUserModelId().map_err(|error| {
                format!("无法读取第 {} 个会话的 Source App ID：{error}", index + 1)
            })?;
            source_app_ids.push(source_app_id.to_string());
        }

        Ok(source_app_ids)
    }
}
