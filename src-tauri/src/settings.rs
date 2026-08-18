use serde::{Deserialize, Serialize};

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
