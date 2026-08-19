use std::{sync::RwLock, time::SystemTime};

use tauri::{AppHandle, Runtime};

use crate::settings::AppSettings;

/// 保存整个应用进程只应存在一份的基础运行状态。
#[derive(Debug)]
pub struct AppState {
    application_version: String,
    started_at: SystemTime,
    settings: RwLock<AppSettings>,
}

impl AppState {
    /// 使用当前应用版本和已读取的设置创建状态，并记录本次进程的启动时间。
    pub fn new(application_version: impl Into<String>, settings: AppSettings) -> Self {
        Self {
            application_version: application_version.into(),
            started_at: SystemTime::now(),
            settings: RwLock::new(settings),
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

    /// 返回当前用户设置的独立副本，避免调用方长期占用读锁。
    pub fn settings(&self) -> Result<AppSettings, String> {
        self.settings
            .read()
            .map(|settings| settings.clone())
            .map_err(|_| "无法读取应用设置：设置状态锁已损坏".to_owned())
    }

    /// 串行保存并替换内存设置，确保多个窗口不会同时写入同一临时文件。
    pub fn update_settings<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        updated_settings: AppSettings,
    ) -> Result<AppSettings, String> {
        let mut current_settings = self
            .settings
            .write()
            .map_err(|_| "无法更新应用设置：设置状态锁已损坏".to_owned())?;

        updated_settings
            .save(app)
            .map_err(|error| format!("无法保存应用设置：{error}"))?;
        *current_settings = updated_settings.clone();

        Ok(updated_settings)
    }
}
