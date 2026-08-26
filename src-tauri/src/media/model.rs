use serde::Serialize;

const MAX_MEDIA_TEXT_CHARS: usize = 4096;

/// 当前 Windows 系统会话提供的媒体元数据；封面与文字始终属于同一份快照。
#[derive(Debug, Clone)]
pub(crate) struct CurrentMediaMetadata {
    pub(crate) source_app_id: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) artwork_data_url: Option<String>,
    pub(crate) accent_color: String,
}

/// Muse Bar 用于会话选择和诊断的播放器类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum MediaPlayerKind {
    #[serde(rename = "qqMusic")]
    QqMusic,
    #[serde(rename = "neteaseCloudMusic")]
    NeteaseCloudMusic,
    #[serde(rename = "kugouMusic")]
    KugouMusic,
    #[serde(rename = "qishuiMusic")]
    QishuiMusic,
    #[serde(rename = "other")]
    Other,
}

impl MediaPlayerKind {
    /// 只有明确支持的播放器才参与 Muse Bar 的活动优先选择。
    pub(crate) fn is_supported(self) -> bool {
        self != Self::Other
    }
}

struct PlayerIdentificationRule {
    player_kind: MediaPlayerKind,
    matches: fn(&str) -> bool,
}

const PLAYER_IDENTIFICATION_RULES: [PlayerIdentificationRule; 4] = [
    PlayerIdentificationRule {
        player_kind: MediaPlayerKind::QqMusic,
        matches: |source| source.contains("qqmusic"),
    },
    PlayerIdentificationRule {
        player_kind: MediaPlayerKind::NeteaseCloudMusic,
        matches: |source| source.contains("cloudmusic") || source.contains("netease"),
    },
    PlayerIdentificationRule {
        player_kind: MediaPlayerKind::KugouMusic,
        matches: |source| source.contains("kugou") || source.contains("kgmusic"),
    },
    PlayerIdentificationRule {
        player_kind: MediaPlayerKind::QishuiMusic,
        matches: |source| {
            source == "汽水音乐"
                || source.contains("qishui")
                || source.contains("com.ss.android.ugc.luna")
                || source.ends_with("luna.exe")
        },
    },
];

/// 按集中维护的识别规则把 Windows Source App ID 映射为播放器类别。
pub(crate) fn identify_media_player(source_app_id: &str) -> MediaPlayerKind {
    let normalized = source_app_id.to_ascii_lowercase();
    PLAYER_IDENTIFICATION_RULES
        .iter()
        .find(|rule| (rule.matches)(&normalized))
        .map_or(MediaPlayerKind::Other, |rule| rule.player_kind)
}

/// 限制播放器提供的标题和歌手长度，避免异常会话把大字符串长期留在缓存。
pub(crate) fn bounded_media_text(value: String) -> String {
    if value.chars().count() <= MAX_MEDIA_TEXT_CHARS {
        return value;
    }
    value.chars().take(MAX_MEDIA_TEXT_CHARS).collect()
}

/// 诊断页面使用的会话来源标识及其识别结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaSessionIdentity {
    pub(crate) session_key: u64,
    pub(crate) source_app_id: String,
    pub(crate) player_kind: MediaPlayerKind,
}

/// Windows 当前媒体会话的播放状态。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CurrentPlaybackStatus {
    Closed,
    Opened,
    Changing,
    Stopped,
    Playing,
    Paused,
    Unknown,
}

/// Windows 当前媒体会话声明支持的控制能力。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentPlaybackCapabilities {
    pub(crate) can_play: bool,
    pub(crate) can_pause: bool,
    pub(crate) can_previous: bool,
    pub(crate) can_next: bool,
    pub(crate) can_seek: bool,
}

/// 播放状态变化使用的轻量事件，不重复携带标题、歌手或封面。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentPlaybackState {
    pub(crate) session_key: u64,
    pub(crate) playback_status: CurrentPlaybackStatus,
    pub(crate) capabilities: CurrentPlaybackCapabilities,
}

/// Windows 当前媒体会话上报的有效时间轴快照。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentTimeline {
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) position_ms: i64,
    pub(crate) min_seek_ms: i64,
    pub(crate) max_seek_ms: i64,
    pub(crate) last_updated_at_unix_ms: i64,
    pub(crate) playback_rate: Option<f64>,
}

/// 前端消费的统一媒体快照，汇总当前会话的显示信息、状态、能力和时间轴。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaSnapshot {
    pub(crate) session_key: u64,
    pub(crate) source_app_id: String,
    pub(crate) player_kind: MediaPlayerKind,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) artwork_data_url: Option<String>,
    pub(crate) accent_color: String,
    pub(crate) system_accent_color: String,
    pub(crate) playback_status: CurrentPlaybackStatus,
    pub(crate) capabilities: CurrentPlaybackCapabilities,
    pub(crate) timeline: Option<CurrentTimeline>,
}
