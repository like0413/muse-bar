use std::{
    error::Error,
    fs::{self, File},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

const SETTINGS_FILE_NAME: &str = "settings.json";
const CURRENT_SETTINGS_SCHEMA_VERSION: u32 = 8;
const WIDTH_SETTINGS_SCHEMA_VERSION: u32 = 7;
const DEFAULT_MIN_WIDTH: u32 = 240;
const DEFAULT_MAX_WIDTH: u32 = 380;
const ALLOWED_MIN_WIDTH: u32 = 200;
const ALLOWED_MAX_WIDTH: u32 = 520;
const DEFAULT_TITLE_SCROLL_SPEED: u32 = 30;
const MINIMUM_TITLE_SCROLL_SPEED: u32 = 10;
const MAXIMUM_TITLE_SCROLL_SPEED: u32 = 100;

/// Bar 窗口与 Windows 任务栏之间的宿主模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowMode {
    /// 使用当前默认宿主策略；现阶段只启用 Child，Owner 回退将在后续版本实现。
    Auto,
    /// 为将来的独立 Owner 宿主预留，现阶段不启用。
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

/// Muse Bar 前端窗口采用的颜色模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    /// 跟随 Windows 当前的应用颜色模式。
    System,
    Dark,
    Light,
}

/// 歌曲标题超出可用宽度时采用的滚动循环方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TitleScrollMode {
    /// 复制标题并首尾衔接，形成不间断的循环滚动。
    Continuous,
    /// 单个标题滚动到末尾后回到起点重新开始。
    Restart,
}

/// Muse Bar 可持久化的用户设置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "initial_settings_schema_version")]
    pub schema_version: u32,
    pub window_mode: WindowMode,
    pub position: TaskbarPosition,
    pub min_width: u32,
    pub max_width: u32,
    pub manual_offset: i32,
    pub progress_style: ProgressStyle,
    pub color_mode: ColorMode,
    pub title_scroll_enabled: bool,
    pub title_scroll_speed: u32,
    pub title_scroll_mode: TitleScrollMode,
    pub launch_on_startup: bool,
}

impl Default for AppSettings {
    /// 返回首版约定的稳定默认设置。
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SETTINGS_SCHEMA_VERSION,
            window_mode: WindowMode::Auto,
            position: TaskbarPosition::Right,
            min_width: DEFAULT_MIN_WIDTH,
            max_width: DEFAULT_MAX_WIDTH,
            manual_offset: 0,
            progress_style: ProgressStyle::Underline,
            color_mode: ColorMode::System,
            title_scroll_enabled: true,
            title_scroll_speed: DEFAULT_TITLE_SCROLL_SPEED,
            title_scroll_mode: TitleScrollMode::Continuous,
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

        match serde_json::from_str::<Self>(&contents) {
            Ok(mut settings) => {
                let migrated = settings.migrate();
                let width_range_normalized = settings.normalize_width_range();
                let title_scroll_speed_normalized = settings.normalize_title_scroll_speed();
                if migrated || width_range_normalized || title_scroll_speed_normalized {
                    // 迁移写回失败不应阻止启动；本次运行仍使用迁移后的内存设置。
                    let _ = settings.save(app);
                }
                Ok(settings)
            }
            Err(_) => {
                preserve_corrupted_file(&settings_path);
                Ok(Self::default())
            }
        }
    }

    /// 将设置完整写入临时文件，再用它替换正式设置文件。
    pub fn save<R: Runtime>(&self, app: &AppHandle<R>) -> Result<(), Box<dyn Error>> {
        let settings_path = settings_file_path(app)?;
        let config_directory = settings_path
            .parent()
            .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "无法确定设置文件所在目录"))?;
        fs::create_dir_all(config_directory)?;

        // 先在内存中完成序列化，避免序列化失败时留下不完整的临时文件。
        let serialized_settings = serde_json::to_vec_pretty(self)?;
        let temporary_path = temporary_settings_path(config_directory);
        write_temporary_settings(&temporary_path, &serialized_settings)?;

        if let Err(error) = fs::rename(&temporary_path, &settings_path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error.into());
        }

        Ok(())
    }

    /// 按设置版本执行一次性迁移，并返回本次是否修改了内容。
    fn migrate(&mut self) -> bool {
        if self.schema_version >= CURRENT_SETTINGS_SCHEMA_VERSION {
            return false;
        }

        // 第 7 版曾调整产品宽度约束；后续无关字段升级不能再次覆盖用户已经保存的宽度。
        if self.schema_version < WIDTH_SETTINGS_SCHEMA_VERSION {
            self.min_width = DEFAULT_MIN_WIDTH;
            self.max_width = DEFAULT_MAX_WIDTH;
        }
        self.schema_version = CURRENT_SETTINGS_SCHEMA_VERSION;
        true
    }

    /// 将宽度限制在产品范围内，并保证最小值不大于最大值。
    pub(crate) fn normalize_width_range(&mut self) -> bool {
        let minimum_width = self.min_width.clamp(ALLOWED_MIN_WIDTH, ALLOWED_MAX_WIDTH);
        let maximum_width = self.max_width.clamp(minimum_width, ALLOWED_MAX_WIDTH);
        let changed = self.min_width != minimum_width || self.max_width != maximum_width;
        self.min_width = minimum_width;
        self.max_width = maximum_width;
        changed
    }

    /// 将标题滚动速度限制在设置页提供的范围内。
    pub(crate) fn normalize_title_scroll_speed(&mut self) -> bool {
        let speed = self
            .title_scroll_speed
            .clamp(MINIMUM_TITLE_SCROLL_SPEED, MAXIMUM_TITLE_SCROLL_SPEED);
        let changed = self.title_scroll_speed != speed;
        self.title_scroll_speed = speed;
        changed
    }
}

/// 配置文件缺少版本字段时，将其视为第一版结构，以便执行升级迁移。
fn initial_settings_schema_version() -> u32 {
    1
}

/// 返回当前应用专属配置目录中的设置文件路径。
fn settings_file_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, tauri::Error> {
    Ok(app.path().app_config_dir()?.join(SETTINGS_FILE_NAME))
}

/// 为本次进程生成与正式文件同目录的临时设置文件路径。
fn temporary_settings_path(config_directory: &Path) -> PathBuf {
    config_directory.join(format!("settings.{}.tmp", process::id()))
}

/// 写入并同步临时设置文件；失败时清理可能存在的不完整文件。
fn write_temporary_settings(path: &Path, contents: &[u8]) -> io::Result<()> {
    let write_result = (|| {
        let mut file = File::create(path)?;
        file.write_all(contents)?;
        file.sync_all()?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(path);
    }

    write_result
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
