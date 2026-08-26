use crate::media_model::MediaPlayerKind;

/// 媒体选择策略所需的最小输入，不携带 WinRT 会话或事件订阅状态。
#[derive(Debug, Clone, Copy)]
pub(crate) struct MediaSelectionCandidate {
    pub(crate) session_key: u64,
    pub(crate) player_kind: MediaPlayerKind,
    pub(crate) is_playing: bool,
    pub(crate) is_paused: bool,
    pub(crate) activity_sequence: Option<u64>,
}

/// 优先选择最近播放的受支持播放器，其次选择最近暂停的受支持播放器。
pub(crate) fn select_preferred_session_key(
    candidates: impl IntoIterator<Item = MediaSelectionCandidate>,
) -> Option<u64> {
    candidates
        .into_iter()
        .filter(|candidate| candidate.player_kind.is_supported())
        .filter_map(|candidate| {
            let playback_priority = if candidate.is_playing {
                2
            } else if candidate.is_paused {
                1
            } else {
                return None;
            };
            candidate
                .activity_sequence
                .map(|sequence| (candidate.session_key, playback_priority, sequence))
        })
        .max_by_key(|(_, playback_priority, sequence)| (*playback_priority, *sequence))
        .map(|(session_key, _, _)| session_key)
}
