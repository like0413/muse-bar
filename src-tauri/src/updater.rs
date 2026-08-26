use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_updater::UpdaterExt;

use crate::app_lifecycle;

pub(crate) const UPDATE_STATUS_EVENT: &str = "updater-status";

/// 更新器对设置页公开的有限状态集合。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateStage {
    Idle,
    Checking,
    Available,
    Downloading,
    Installing,
    UpToDate,
    Error,
}

/// 更新检查、下载进度和错误信息的可序列化快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    stage: UpdateStage,
    current_version: String,
    available_version: Option<String>,
    notes: Option<String>,
    published_at: Option<String>,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    error: Option<String>,
}

impl UpdateStatus {
    fn idle(current_version: String) -> Self {
        Self {
            stage: UpdateStage::Idle,
            current_version,
            available_version: None,
            notes: None,
            published_at: None,
            downloaded_bytes: 0,
            total_bytes: None,
            error: None,
        }
    }
}

/// 串行化更新操作，并保存跨设置窗口生命周期存在的更新状态。
pub struct UpdateManager {
    status: Mutex<UpdateStatus>,
    operation_in_progress: AtomicBool,
}

impl UpdateManager {
    pub fn new(current_version: String) -> Self {
        Self {
            status: Mutex::new(UpdateStatus::idle(current_version)),
            operation_in_progress: AtomicBool::new(false),
        }
    }

    pub fn snapshot(&self) -> Result<UpdateStatus, String> {
        self.status
            .lock()
            .map(|status| status.clone())
            .map_err(|_| "无法读取更新状态：状态锁已损坏".to_owned())
    }

    fn replace(&self, status: UpdateStatus) -> Result<UpdateStatus, String> {
        let mut current = self
            .status
            .lock()
            .map_err(|_| "无法更新更新状态：状态锁已损坏".to_owned())?;
        *current = status.clone();
        Ok(status)
    }

    fn begin_operation(&self) -> Result<UpdateOperation<'_>, String> {
        self.operation_in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "已有更新操作正在进行".to_owned())?;
        Ok(UpdateOperation(&self.operation_in_progress))
    }

    fn set_stage(&self, stage: UpdateStage) -> Result<UpdateStatus, String> {
        let mut status = self.snapshot()?;
        status.stage = stage;
        status.error = None;
        self.replace(status)
    }

    fn set_error(&self, message: String) -> Result<UpdateStatus, String> {
        let mut status = self.snapshot()?;
        status.stage = UpdateStage::Error;
        status.downloaded_bytes = 0;
        status.total_bytes = None;
        status.error = Some(message);
        self.replace(status)
    }

    fn record_download(
        &self,
        chunk_length: usize,
        total: Option<u64>,
    ) -> Result<UpdateStatus, String> {
        let mut status = self.snapshot()?;
        status.stage = UpdateStage::Downloading;
        status.downloaded_bytes = status
            .downloaded_bytes
            .saturating_add(u64::try_from(chunk_length).unwrap_or(u64::MAX));
        status.total_bytes = total;
        self.replace(status)
    }
}

struct UpdateOperation<'a>(&'a AtomicBool);

impl Drop for UpdateOperation<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// 在应用启动后执行一次非阻塞检查；网络失败只写日志，不打扰用户。
pub fn start_automatic_check<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = check_for_update(&app, true).await {
            log::warn!("启动时检查更新失败：{error}");
        }
    });
}

/// 检查 GitHub Release，并按是否为自动检查决定错误是否展示给用户。
pub async fn check_for_update<R: Runtime>(
    app: &AppHandle<R>,
    automatic: bool,
) -> Result<UpdateStatus, String> {
    let manager = app.state::<UpdateManager>();
    let _operation = manager.begin_operation()?;
    emit_status(app, manager.set_stage(UpdateStage::Checking)?);

    let result = async {
        let update = app
            .updater()
            .map_err(|error| format!("无法初始化更新器：{error}"))?
            .check()
            .await
            .map_err(|error| format!("无法检查更新：{error}"))?;

        let current_version = app.package_info().version.to_string();
        let status = match update {
            Some(update) => UpdateStatus {
                stage: UpdateStage::Available,
                current_version,
                available_version: Some(update.version),
                notes: update.body.filter(|notes| !notes.trim().is_empty()),
                published_at: update.date.map(|date| date.to_string()),
                downloaded_bytes: 0,
                total_bytes: None,
                error: None,
            },
            None => UpdateStatus {
                stage: UpdateStage::UpToDate,
                current_version,
                available_version: None,
                notes: None,
                published_at: None,
                downloaded_bytes: 0,
                total_bytes: None,
                error: None,
            },
        };
        manager.replace(status)
    }
    .await;

    match result {
        Ok(status) => {
            emit_status(app, status.clone());
            if automatic && matches!(status.stage, UpdateStage::Available) {
                open_settings_for_update(app);
            }
            Ok(status)
        }
        Err(error) => {
            let status = if automatic {
                manager.replace(UpdateStatus::idle(app.package_info().version.to_string()))?
            } else {
                manager.set_error(error.clone())?
            };
            emit_status(app, status);
            Err(error)
        }
    }
}

/// 重新读取最新 Release，确认版本未变化后下载、验签并交给 NSIS 安装。
pub async fn install_update<R: Runtime>(
    app: &AppHandle<R>,
    expected_version: &str,
) -> Result<(), String> {
    let manager = app.state::<UpdateManager>();
    let _operation = manager.begin_operation()?;
    emit_status(app, manager.set_stage(UpdateStage::Checking)?);

    let result = async {
        let update = app
            .updater()
            .map_err(|error| format!("无法初始化更新器：{error}"))?
            .check()
            .await
            .map_err(|error| format!("无法重新确认更新：{error}"))?
            .ok_or_else(|| "该更新已不可用，请重新检查".to_owned())?;

        if update.version != expected_version {
            return Err(format!(
                "最新版本已从 {expected_version} 变为 {}，请确认后重试",
                update.version
            ));
        }

        let mut status = manager.snapshot()?;
        status.stage = UpdateStage::Downloading;
        status.downloaded_bytes = 0;
        status.total_bytes = None;
        status.error = None;
        emit_status(app, manager.replace(status)?);

        let progress_app = app.clone();
        let finished_app = app.clone();
        update
            .download_and_install(
                move |chunk_length, total| {
                    let progress_manager = progress_app.state::<UpdateManager>();
                    match progress_manager.record_download(chunk_length, total) {
                        Ok(status) => emit_status(&progress_app, status),
                        Err(error) => log::error!("无法记录更新下载进度：{error}"),
                    }
                },
                move || {
                    let finished_manager = finished_app.state::<UpdateManager>();
                    match finished_manager.set_stage(UpdateStage::Installing) {
                        Ok(status) => emit_status(&finished_app, status),
                        Err(error) => log::error!("无法记录更新安装状态：{error}"),
                    }
                },
            )
            .await
            .map_err(|error| format!("无法安装更新：{error}"))
    }
    .await;

    if let Err(error) = result {
        emit_status(app, manager.set_error(error.clone())?);
        return Err(error);
    }

    Ok(())
}

fn emit_status<R: Runtime>(app: &AppHandle<R>, status: UpdateStatus) {
    if let Err(error) = app.emit_to(
        app_lifecycle::SETTINGS_WINDOW_LABEL,
        UPDATE_STATUS_EVENT,
        status,
    ) {
        log::debug!("设置窗口尚未监听更新状态：{error}");
    }
}

fn open_settings_for_update<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    if let Err(error) = app.clone().run_on_main_thread(move || {
        if let Err(error) = app_lifecycle::open_settings_window(&app) {
            log::error!("发现更新后无法打开设置窗口：{error}");
        }
    }) {
        log::error!("发现更新后无法调度设置窗口：{error}");
    }
}
