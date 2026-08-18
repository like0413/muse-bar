use std::time::SystemTime;

/// 保存整个应用进程只应存在一份的基础运行状态。
#[derive(Debug)]
pub struct AppState {
    application_version: String,
    started_at: SystemTime,
}

impl AppState {
    /// 使用当前应用版本创建状态，并记录本次进程的启动时间。
    pub fn new(application_version: impl Into<String>) -> Self {
        Self {
            application_version: application_version.into(),
            started_at: SystemTime::now(),
        }
    }

    /// 返回本次运行对应的应用版本。
    pub fn application_version(&self) -> &str {
        &self.application_version
    }

    /// 返回状态创建时记录的进程启动时间。
    pub fn started_at(&self) -> SystemTime {
        self.started_at
    }
}
