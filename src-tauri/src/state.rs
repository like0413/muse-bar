use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::SystemTime,
};

use tauri::{AppHandle, Runtime};

use crate::settings::AppSettings;

/// Bar 前端最近一次上报的逻辑宽度及其设置边界计算结果。
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarWidthMeasurement {
    natural_width: f64,
    target_width: u32,
    minimum_width: u32,
    maximum_width: u32,
}

impl BarWidthMeasurement {
    /// 返回设置边界限制后的目标逻辑宽度。
    pub fn target_width(&self) -> u32 {
        self.target_width
    }
}

/// 保存整个应用进程只应存在一份的基础运行状态。
#[derive(Debug)]
pub struct AppState {
    application_version: String,
    started_at: SystemTime,
    settings: RwLock<AppSettings>,
    bar_width_measurement: RwLock<Option<BarWidthMeasurement>>,
    bar_width_animation_revision: Arc<AtomicU64>,
    bar_visible: AtomicBool,
}

impl AppState {
    /// 使用当前应用版本和已读取的设置创建状态，并记录本次进程的启动时间。
    pub fn new(application_version: impl Into<String>, settings: AppSettings) -> Self {
        Self {
            application_version: application_version.into(),
            started_at: SystemTime::now(),
            settings: RwLock::new(settings),
            bar_width_measurement: RwLock::new(None),
            bar_width_animation_revision: Arc::new(AtomicU64::new(0)),
            bar_visible: AtomicBool::new(true),
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
        mut updated_settings: AppSettings,
    ) -> Result<AppSettings, String> {
        let mut current_settings = self
            .settings
            .write()
            .map_err(|_| "无法更新应用设置：设置状态锁已损坏".to_owned())?;

        updated_settings.normalize_width_range();
        updated_settings
            .save(app)
            .map_err(|error| format!("无法保存应用设置：{error}"))?;
        *current_settings = updated_settings.clone();

        Ok(updated_settings)
    }

    /// 使用当前设置限制前端测得的自然宽度，并保存供后续原生窗口调整使用的目标值。
    pub fn report_bar_content_width(
        &self,
        natural_width: f64,
    ) -> Result<BarWidthMeasurement, String> {
        if !natural_width.is_finite() || natural_width <= 0.0 || natural_width > f64::from(u32::MAX)
        {
            return Err("Bar 上报的内容宽度无效".to_owned());
        }

        let settings = self.settings()?;
        let minimum_width = settings.min_width;
        let maximum_width = settings.max_width;
        let rounded_width = natural_width.ceil() as u32;
        let measurement = BarWidthMeasurement {
            natural_width,
            target_width: rounded_width.clamp(minimum_width, maximum_width),
            minimum_width,
            maximum_width,
        };
        let mut current_measurement = self
            .bar_width_measurement
            .write()
            .map_err(|_| "无法保存 Bar 宽度：宽度状态锁已损坏".to_owned())?;
        *current_measurement = Some(measurement);

        Ok(measurement)
    }

    /// 生成新宽度动画的版本号；旧动画发现版本落后后会停止提交窗口变化。
    pub fn begin_bar_width_animation(&self) -> (u64, Arc<AtomicU64>) {
        let revision = self
            .bar_width_animation_revision
            .fetch_add(1, Ordering::AcqRel)
            + 1;
        (revision, Arc::clone(&self.bar_width_animation_revision))
    }

    /// 返回用户在本次运行中选择的临时 Bar 显示状态。
    pub fn is_bar_visible(&self) -> bool {
        self.bar_visible.load(Ordering::Acquire)
    }

    /// 原子切换临时 Bar 显示状态，并返回切换后的值。
    pub fn toggle_bar_visibility(&self) -> bool {
        !self.bar_visible.fetch_xor(true, Ordering::AcqRel)
    }
}
