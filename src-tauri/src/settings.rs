use std::{
    error::Error,
    fs::{self, File},
    io::ErrorKind,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

const SETTINGS_FILE_NAME: &str = "settings.json";

/// Bar 窗口与 Windows 任务栏之间的宿主模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowMode {
    /// 优先尝试 Child 嵌入，失败时回退到 Owner 模式。
    Auto,
    /// 始终使用独立的 Owner 窗口贴合任务栏。
    Owner,
}

/// Bar 在任务栏可用区域中的目标位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskbarPosition {
    Left,
    Center,
    Right,
}

/// 播放进度在 Bar 中的显示方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProgressStyle {
    /// 在 Bar 底部显示一条细进度线。
    Underline,
    /// 使用由弱到强的背景渐变表示已播放范围。
    BackgroundGradient,
}

/// Muse Bar 可持久化的用户设置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    pub window_mode: WindowMode,
    pub position: TaskbarPosition,
    pub min_width: u32,
    pub max_width: u32,
    pub manual_offset: i32,
    pub progress_style: ProgressStyle,
    pub launch_on_startup: bool,
}

impl Default for AppSettings {
    /// 返回首版约定的稳定默认设置。
    fn default() -> Self {
        Self {
            window_mode: WindowMode::Auto,
            position: TaskbarPosition::Right,
            min_width: 320,
            max_width: 560,
            manual_offset: 0,
            progress_style: ProgressStyle::Underline,
            launch_on_startup: false,
        }
    }
}

impl AppSettings {
    /// 从应用配置目录读取设置；首次启动尚无设置文件时返回默认值。
    pub fn load<R: Runtime>(app: &AppHandle<R>) -> Result<Self, Box<dyn Error>> {
        let settings_path = settings_file_path(app)?;

        let contents = match fs::read_to_string(&settings_path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error.into()),
        };

        match serde_json::from_str(&contents) {
            Ok(settings) => Ok(settings),
            Err(_) => {
                preserve_corrupted_file(&settings_path);
                Ok(Self::default())
            }
        }
    }

    /// 将设置完整写入临时文件，再用它替换正式设置文件。
    pub fn save<R: Runtime>(&self, app: &AppHandle<R>) -> Result<(), Box<dyn Error>> {
        let settings_path = settings_file_path(app)?;
        let config_directory = settings_path.parent().ok_or_else(|| {
            std::io::Error::new(ErrorKind::InvalidInput, "无法确定设置文件所在目录")
        })?;
        fs::create_dir_all(config_directory)?;

        let temporary_path = temporary_settings_path(config_directory);
        let mut temporary_file = File::create(&temporary_path)?;
        serde_json::to_writer_pretty(&mut temporary_file, self)?;
        temporary_file.sync_all()?;

        if let Err(error) = fs::rename(&temporary_path, &settings_path) {
            let _ = fs::remove_file(temporary_path);
            return Err(error.into());
        }

        Ok(())
    }
}

/// 返回当前应用专属配置目录中的设置文件路径。
fn settings_file_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, tauri::Error> {
    Ok(app.path().app_config_dir()?.join(SETTINGS_FILE_NAME))
}

/// 为本次进程生成与正式文件同目录的临时设置文件路径。
fn temporary_settings_path(config_directory: &Path) -> PathBuf {
    config_directory.join(format!("settings.{}.tmp", process::id()))
}

/// 尝试为损坏的设置文件生成不会覆盖正常配置的备份名称。
fn corrupted_settings_path(settings_path: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());

    settings_path.with_file_name(format!(
        "settings.corrupted-{timestamp}-{}.json",
        process::id()
    ))
}

/// 尝试移动损坏文件；移动失败时保留原文件并继续使用默认设置启动。
fn preserve_corrupted_file(settings_path: &Path) {
    let corrupted_path = corrupted_settings_path(settings_path);
    let _ = fs::rename(settings_path, corrupted_path);
}
