use std::{
    fs::{self, OpenOptions},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};
use thiserror::Error;

const SETTINGS_FILE_NAME: &str = "settings.json";
const CURRENT_SETTINGS_SCHEMA_VERSION: u32 = 12;
const MAXIMUM_CORRUPTED_SETTINGS_BACKUPS: usize = 3;
const DEFAULT_MAX_WIDTH: u32 = 380;
const MINIMUM_ALLOWED_MAX_WIDTH: u32 = 200;
const ALLOWED_MAX_WIDTH: u32 = 520;
const DEFAULT_TITLE_SCROLL_SPEED: u32 = 30;
const MINIMUM_TITLE_SCROLL_SPEED: u32 = 10;
const MAXIMUM_TITLE_SCROLL_SPEED: u32 = 100;
const DEFAULT_TARGET_MONITOR: &str = "primary";
const DEFAULT_CUSTOM_PROGRESS_COLOR: &str = "#0078D4";
const MINIMUM_MANUAL_OFFSET: i32 = -200;
const MAXIMUM_MANUAL_OFFSET: i32 = 200;
static SETTINGS_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Error)]
pub(crate) enum SettingsPersistenceError {
    #[error("设置文件 I/O 失败：{0}")]
    Io(#[from] io::Error),
    #[error("设置 JSON 处理失败：{0}")]
    Json(#[from] serde_json::Error),
    #[error("无法定位应用配置目录：{0}")]
    AppPath(#[from] tauri::Error),
    #[error("无法确定设置文件所在目录")]
    MissingParentDirectory,
    #[error("设置版本 {found} 高于当前程序支持的版本 {supported}，已停止加载以避免覆盖新版本配置")]
    UnsupportedSchemaVersion { found: u64, supported: u32 },
    #[error("schemaVersion 必须是非负整数")]
    InvalidSchemaVersion,
    #[error("设置写入后的校验结果与待保存内容不一致")]
    VerificationMismatch,
}

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
    #[serde(alias = "center")]
    Right,
}

/// 歌词文本在可用内容区域中的水平对齐方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LyricsAlignment {
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
    /// 标题到达末尾后反向移动，回到起点后再次正向滚动。
    Bounce,
}

/// 封面、媒体文字、歌词和控制按钮在 Bar 中的整体排列方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElementAlignment {
    Left,
    Right,
}

/// 进度视觉使用的颜色来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProgressColorSource {
    Artwork,
    System,
    Custom,
}

/// Muse Bar 可持久化的用户设置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "initial_settings_schema_version")]
    pub schema_version: u32,
    pub window_mode: WindowMode,
    pub position: TaskbarPosition,
    pub target_monitor: String,
    pub max_width: u32,
    pub manual_offset: i32,
    pub show_controls: bool,
    pub element_alignment: ElementAlignment,
    pub show_progress: bool,
    pub progress_style: ProgressStyle,
    pub progress_color_source: ProgressColorSource,
    pub custom_progress_color: String,
    pub color_mode: ColorMode,
    pub title_scroll_enabled: bool,
    pub title_scroll_speed: u32,
    pub title_scroll_mode: TitleScrollMode,
    pub lyrics_enabled: bool,
    pub lyrics_alignment: LyricsAlignment,
    pub launch_on_startup: bool,
}

impl Default for AppSettings {
    /// 返回首版约定的稳定默认设置。
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SETTINGS_SCHEMA_VERSION,
            window_mode: WindowMode::Auto,
            position: TaskbarPosition::Right,
            target_monitor: DEFAULT_TARGET_MONITOR.to_owned(),
            max_width: DEFAULT_MAX_WIDTH,
            manual_offset: 0,
            show_controls: true,
            element_alignment: ElementAlignment::Left,
            show_progress: true,
            progress_style: ProgressStyle::Underline,
            progress_color_source: ProgressColorSource::Artwork,
            custom_progress_color: DEFAULT_CUSTOM_PROGRESS_COLOR.to_owned(),
            color_mode: ColorMode::System,
            title_scroll_enabled: true,
            title_scroll_speed: DEFAULT_TITLE_SCROLL_SPEED,
            title_scroll_mode: TitleScrollMode::Continuous,
            lyrics_enabled: false,
            lyrics_alignment: LyricsAlignment::Center,
            launch_on_startup: false,
        }
    }
}

impl AppSettings {
    /// 从应用配置目录读取设置；首次启动尚无设置文件时返回默认值。
    pub fn load<R: Runtime>(app: &AppHandle<R>) -> Result<Self, SettingsPersistenceError> {
        Self::load_from_path(&settings_file_path(app)?)
    }

    fn load_from_path(settings_path: &Path) -> Result<Self, SettingsPersistenceError> {
        cleanup_stale_temporary_files(settings_path);

        let contents = match fs::read_to_string(settings_path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error.into()),
        };

        match deserialize_settings(&contents) {
            Ok(mut settings) => {
                if settings.prepare_for_persistence()? {
                    // 写回失败不阻止启动，本次运行仍使用已经迁移和归一化的内存值。
                    if let Err(error) = settings.save_to_path(settings_path) {
                        log::warn!("设置迁移或归一化结果无法写回：{error}");
                    }
                }
                Ok(settings)
            }
            Err(error @ SettingsPersistenceError::UnsupportedSchemaVersion { .. }) => Err(error),
            Err(error) => {
                let backup_path = preserve_corrupted_file(settings_path)?;
                log::warn!(
                    "设置文件无法解析，已备份至 {} 并恢复默认值：{error}",
                    backup_path.display()
                );
                Ok(Self::default())
            }
        }
    }

    /// 将设置完整写入临时文件，再用它替换正式设置文件。
    pub fn save<R: Runtime>(&self, app: &AppHandle<R>) -> Result<(), SettingsPersistenceError> {
        self.save_to_path(&settings_file_path(app)?)
    }

    fn save_to_path(&self, settings_path: &Path) -> Result<(), SettingsPersistenceError> {
        if u64::from(self.schema_version) > u64::from(CURRENT_SETTINGS_SCHEMA_VERSION) {
            return Err(SettingsPersistenceError::UnsupportedSchemaVersion {
                found: u64::from(self.schema_version),
                supported: CURRENT_SETTINGS_SCHEMA_VERSION,
            });
        }

        let config_directory = settings_path
            .parent()
            .ok_or(SettingsPersistenceError::MissingParentDirectory)?;
        fs::create_dir_all(config_directory)?;

        // 先在内存中完成序列化，避免序列化失败时留下不完整的临时文件。
        let serialized_settings = serde_json::to_vec_pretty(self)?;
        let temporary_path = write_temporary_settings(config_directory, &serialized_settings)?;

        if let Err(error) = fs::rename(&temporary_path, settings_path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error.into());
        }

        let persisted_settings: Self = serde_json::from_slice(&fs::read(settings_path)?)?;
        if persisted_settings != *self {
            return Err(SettingsPersistenceError::VerificationMismatch);
        }

        Ok(())
    }

    /// 执行真实结构迁移和边界归一化；新增可选字段不需要提升 schema 版本。
    pub(crate) fn prepare_for_persistence(&mut self) -> Result<bool, SettingsPersistenceError> {
        if self.schema_version > CURRENT_SETTINGS_SCHEMA_VERSION {
            return Err(SettingsPersistenceError::UnsupportedSchemaVersion {
                found: u64::from(self.schema_version),
                supported: CURRENT_SETTINGS_SCHEMA_VERSION,
            });
        }

        let migrated = if self.schema_version < CURRENT_SETTINGS_SCHEMA_VERSION {
            self.schema_version = CURRENT_SETTINGS_SCHEMA_VERSION;
            true
        } else {
            false
        };

        Ok(self.normalize_maximum_width()
            | self.normalize_title_scroll_speed()
            | self.normalize_positioning()
            | self.normalize_custom_progress_color()
            | migrated)
    }

    /// 将普通模式的最大宽度限制在设置页允许的产品范围内。
    pub(crate) fn normalize_maximum_width(&mut self) -> bool {
        let maximum_width = self
            .max_width
            .clamp(MINIMUM_ALLOWED_MAX_WIDTH, ALLOWED_MAX_WIDTH);
        let changed = self.max_width != maximum_width;
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

    /// 清理显示器标识并限制手动偏移，防止损坏配置产生不可见窗口。
    pub(crate) fn normalize_positioning(&mut self) -> bool {
        let target_monitor = self.target_monitor.trim();
        let normalized_monitor = if target_monitor.is_empty() {
            DEFAULT_TARGET_MONITOR.to_owned()
        } else {
            target_monitor.to_owned()
        };
        let manual_offset = self
            .manual_offset
            .clamp(MINIMUM_MANUAL_OFFSET, MAXIMUM_MANUAL_OFFSET);
        let changed =
            self.target_monitor != normalized_monitor || self.manual_offset != manual_offset;
        self.target_monitor = normalized_monitor;
        self.manual_offset = manual_offset;
        changed
    }

    /// 校验自定义进度颜色，并统一保存为大写六位十六进制格式。
    pub(crate) fn normalize_custom_progress_color(&mut self) -> bool {
        let normalized = normalize_hex_color(&self.custom_progress_color)
            .unwrap_or_else(|| DEFAULT_CUSTOM_PROGRESS_COLOR.to_owned());
        let changed = self.custom_progress_color != normalized;
        self.custom_progress_color = normalized;
        changed
    }
}

/// 接受带井号的六位十六进制颜色，并返回统一的大写格式。
fn normalize_hex_color(value: &str) -> Option<String> {
    let value = value.trim();
    let digits = value.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }

    Some(format!("#{}", digits.to_ascii_uppercase()))
}

/// 配置文件缺少版本字段时，将其视为第一版结构，以便执行升级迁移。
fn initial_settings_schema_version() -> u32 {
    1
}

fn deserialize_settings(contents: &str) -> Result<AppSettings, SettingsPersistenceError> {
    let document: Value = serde_json::from_str(contents)?;
    let schema_version = match document.get("schemaVersion") {
        Some(Value::Number(version)) => version
            .as_u64()
            .ok_or(SettingsPersistenceError::InvalidSchemaVersion)?,
        Some(_) => return Err(SettingsPersistenceError::InvalidSchemaVersion),
        None => u64::from(initial_settings_schema_version()),
    };

    if schema_version > u64::from(CURRENT_SETTINGS_SCHEMA_VERSION) {
        return Err(SettingsPersistenceError::UnsupportedSchemaVersion {
            found: schema_version,
            supported: CURRENT_SETTINGS_SCHEMA_VERSION,
        });
    }

    Ok(serde_json::from_value(document)?)
}

/// 返回当前应用专属配置目录中的设置文件路径。
fn settings_file_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, tauri::Error> {
    Ok(app.path().app_config_dir()?.join(SETTINGS_FILE_NAME))
}

fn unique_file_suffix() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = SETTINGS_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp}-{}-{sequence}", process::id())
}

/// 写入并同步临时设置文件；失败时清理可能存在的不完整文件。
fn write_temporary_settings(config_directory: &Path, contents: &[u8]) -> io::Result<PathBuf> {
    let path = config_directory.join(format!("settings.{}.tmp", unique_file_suffix()));
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(contents)?;
        file.sync_all()?;
        Ok(path.clone())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&path);
    }

    write_result
}

fn cleanup_stale_temporary_files(settings_path: &Path) {
    let Some(config_directory) = settings_path.parent() else {
        return;
    };
    let Ok(entries) = fs::read_dir(config_directory) else {
        return;
    };

    for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.starts_with("settings.")
            && file_name.ends_with(".tmp")
            && fs::remove_file(&path).is_err()
        {
            log::warn!("无法清理残留设置临时文件：{}", path.display());
        }
    }
}

/// 尝试为损坏的设置文件生成不会覆盖正常配置的备份名称。
fn corrupted_settings_path(settings_path: &Path) -> PathBuf {
    settings_path.with_file_name(format!("settings.corrupted-{}.json", unique_file_suffix()))
}

/// 移动损坏文件并只保留最近几份，避免无限占用用户配置目录。
fn preserve_corrupted_file(settings_path: &Path) -> Result<PathBuf, SettingsPersistenceError> {
    let corrupted_path = corrupted_settings_path(settings_path);
    fs::rename(settings_path, &corrupted_path)?;
    prune_corrupted_settings_backups(settings_path);
    Ok(corrupted_path)
}

fn prune_corrupted_settings_backups(settings_path: &Path) {
    let Some(config_directory) = settings_path.parent() else {
        return;
    };
    let Ok(entries) = fs::read_dir(config_directory) else {
        return;
    };
    let mut backups = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("settings.corrupted-") && name.ends_with(".json")
                })
        })
        .collect::<Vec<_>>();
    backups.sort_unstable_by(|left, right| right.file_name().cmp(&left.file_name()));

    for path in backups.into_iter().skip(MAXIMUM_CORRUPTED_SETTINGS_BACKUPS) {
        if fs::remove_file(&path).is_err() {
            log::warn!("无法清理旧的损坏设置备份：{}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "muse-bar-settings-{label}-{}",
                unique_file_suffix()
            ));
            fs::create_dir_all(&path).expect("测试配置目录应能创建");
            Self(path)
        }

        fn settings_path(&self) -> PathBuf {
            self.0.join(SETTINGS_FILE_NAME)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn save_to_path_should_replace_existing_settings_file() {
        let directory = TestDirectory::new("replace-existing");
        let settings_path = directory.settings_path();
        fs::write(&settings_path, b"old contents").expect("旧设置文件应能写入");
        let settings = AppSettings {
            max_width: 412,
            ..AppSettings::default()
        };

        settings
            .save_to_path(&settings_path)
            .expect("已有设置文件应能被原子替换");

        let persisted: AppSettings =
            serde_json::from_slice(&fs::read(settings_path).expect("替换后的设置文件应能读取"))
                .expect("替换后的设置应为有效 JSON");
        assert_eq!(persisted.max_width, 412);
    }

    #[test]
    fn save_to_path_should_leave_no_temporary_file_after_success() {
        let directory = TestDirectory::new("no-temporary-file");

        AppSettings::default()
            .save_to_path(&directory.settings_path())
            .expect("默认设置应能保存");

        let has_temporary_file = fs::read_dir(&directory.0)
            .expect("测试配置目录应能读取")
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"));
        assert!(!has_temporary_file, "保存成功后不应残留临时文件");
    }

    #[cfg(windows)]
    #[test]
    fn save_to_path_should_preserve_existing_file_when_replacement_is_blocked() {
        use std::os::windows::fs::OpenOptionsExt;

        let directory = TestDirectory::new("blocked-replacement-preserves-original");
        let settings_path = directory.settings_path();
        fs::write(&settings_path, b"original contents").expect("原设置文件应能写入");
        let locked_file = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&settings_path)
            .expect("原设置文件应能被独占打开");

        let result = AppSettings::default().save_to_path(&settings_path);
        drop(locked_file);

        assert!(result.is_err(), "目标被占用时保存必须返回错误");
        assert_eq!(
            fs::read(settings_path).expect("保存失败后原文件应仍能读取"),
            b"original contents"
        );
    }

    #[cfg(windows)]
    #[test]
    fn save_to_path_should_cleanup_temporary_file_when_replacement_is_blocked() {
        use std::os::windows::fs::OpenOptionsExt;

        let directory = TestDirectory::new("blocked-replacement-cleans-temporary");
        let settings_path = directory.settings_path();
        fs::write(&settings_path, b"original contents").expect("原设置文件应能写入");
        let locked_file = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&settings_path)
            .expect("原设置文件应能被独占打开");

        let _ = AppSettings::default().save_to_path(&settings_path);
        drop(locked_file);

        let has_temporary_file = fs::read_dir(&directory.0)
            .expect("测试配置目录应能读取")
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"));
        assert!(!has_temporary_file, "替换失败后不应残留临时文件");
    }

    #[test]
    fn load_from_path_should_remove_stale_temporary_files() {
        let directory = TestDirectory::new("stale-temporary-file");
        let stale_path = directory.0.join("settings.stale.tmp");
        fs::write(&stale_path, b"partial").expect("残留临时文件应能创建");

        AppSettings::load_from_path(&directory.settings_path())
            .expect("缺少正式设置文件时应使用默认值");

        assert!(!stale_path.exists(), "加载时应清理残留临时文件");
    }

    #[test]
    fn load_from_path_should_fill_missing_additive_fields_from_defaults() {
        let directory = TestDirectory::new("additive-defaults");
        fs::write(
            directory.settings_path(),
            format!(r#"{{"schemaVersion":{CURRENT_SETTINGS_SCHEMA_VERSION},"maxWidth":412}}"#),
        )
        .expect("精简设置文件应能写入");

        let settings = AppSettings::load_from_path(&directory.settings_path())
            .expect("缺少新增字段时应使用默认值");

        assert!(settings.show_controls, "缺少字段应采用当前默认值");
    }

    #[test]
    fn load_from_path_should_write_current_schema_after_legacy_load() {
        let directory = TestDirectory::new("legacy-schema");
        fs::write(
            directory.settings_path(),
            r#"{"schemaVersion":1,"maxWidth":412}"#,
        )
        .expect("旧版设置文件应能写入");

        AppSettings::load_from_path(&directory.settings_path()).expect("旧版设置应能迁移");

        let document: Value = serde_json::from_slice(
            &fs::read(directory.settings_path()).expect("迁移后的设置文件应能读取"),
        )
        .expect("迁移后的设置应为有效 JSON");
        assert_eq!(
            document.get("schemaVersion").and_then(Value::as_u64),
            Some(u64::from(CURRENT_SETTINGS_SCHEMA_VERSION))
        );
    }

    #[test]
    fn load_from_path_should_reject_future_schema() {
        let directory = TestDirectory::new("future-schema-error");
        let future_version = u64::from(CURRENT_SETTINGS_SCHEMA_VERSION) + 1;
        fs::write(
            directory.settings_path(),
            format!(r#"{{"schemaVersion":{future_version},"futureField":true}}"#),
        )
        .expect("未来版本设置文件应能写入");

        let error = AppSettings::load_from_path(&directory.settings_path())
            .expect_err("未来版本设置必须拒绝加载");

        assert!(matches!(
            error,
            SettingsPersistenceError::UnsupportedSchemaVersion { .. }
        ));
    }

    #[test]
    fn load_from_path_should_not_modify_future_schema_file() {
        let directory = TestDirectory::new("future-schema-preserved");
        let original = format!(
            r#"{{"schemaVersion":{},"futureField":true}}"#,
            CURRENT_SETTINGS_SCHEMA_VERSION + 1
        );
        fs::write(directory.settings_path(), &original).expect("未来版本设置文件应能写入");

        let _ = AppSettings::load_from_path(&directory.settings_path());

        let persisted =
            fs::read_to_string(directory.settings_path()).expect("拒绝加载后原设置文件应仍然存在");
        assert_eq!(persisted, original);
    }

    #[test]
    fn load_from_path_should_backup_invalid_json() {
        let directory = TestDirectory::new("invalid-json-backup");
        fs::write(directory.settings_path(), b"not json").expect("损坏设置文件应能写入");

        AppSettings::load_from_path(&directory.settings_path())
            .expect("损坏设置应备份并恢复默认值");

        let backup_count = corrupted_backup_count(&directory.0);
        assert_eq!(backup_count, 1);
    }

    #[test]
    fn load_from_path_should_keep_at_most_three_corrupted_backups() {
        let directory = TestDirectory::new("backup-limit");
        for index in 0..5 {
            fs::write(
                directory
                    .0
                    .join(format!("settings.corrupted-000{index}.json")),
                b"old backup",
            )
            .expect("旧损坏备份应能写入");
        }
        fs::write(directory.settings_path(), b"not json").expect("损坏设置文件应能写入");

        AppSettings::load_from_path(&directory.settings_path())
            .expect("损坏设置应备份并恢复默认值");

        assert_eq!(
            corrupted_backup_count(&directory.0),
            MAXIMUM_CORRUPTED_SETTINGS_BACKUPS
        );
    }

    fn corrupted_backup_count(directory: &Path) -> usize {
        fs::read_dir(directory)
            .expect("测试配置目录应能读取")
            .filter_map(Result::ok)
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.starts_with("settings.corrupted-") && name.ends_with(".json")
            })
            .count()
    }
}
