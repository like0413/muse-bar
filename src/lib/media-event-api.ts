import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import type {
  CurrentTimeline,
  CurrentPlaybackState,
  MediaSessionActivity,
  MediaSessionIdentity,
  MediaSnapshot,
} from './media-types'

const MEDIA_SESSION_IDENTITIES_CHANGED_EVENT = 'media-session-identities-changed'
const MEDIA_SESSION_ACTIVITIES_CHANGED_EVENT = 'media-session-activities-changed'
const CURRENT_TIMELINE_CHANGED_EVENT = 'current-timeline-changed'
const CURRENT_PLAYBACK_STATE_CHANGED_EVENT = 'current-playback-state-changed'
const CURRENT_MEDIA_SNAPSHOT_CHANGED_EVENT = 'current-media-snapshot-changed'

/** 订阅媒体会话身份变化，并返回用于取消订阅的函数。 */
export function listenToMediaSessionIdentityChanges(
  handleIdentities: (identities: MediaSessionIdentity[]) => void,
): Promise<UnlistenFn> {
  return listen<MediaSessionIdentity[]>(MEDIA_SESSION_IDENTITIES_CHANGED_EVENT, (event) => {
    handleIdentities(event.payload)
  })
}

/** 订阅播放开始、切歌或 CurrentSession 变化产生的活动记录。 */
export function listenToMediaSessionActivityChanges(
  handleActivities: (activities: MediaSessionActivity[]) => void,
): Promise<UnlistenFn> {
  return listen<MediaSessionActivity[]>(MEDIA_SESSION_ACTIVITIES_CHANGED_EVENT, (event) => {
    handleActivities(event.payload)
  })
}

/** 订阅 Windows 当前媒体会话时间轴变化。 */
export function listenToCurrentTimelineChanges(
  handleTimeline: (timeline: CurrentTimeline | null) => void,
): Promise<UnlistenFn> {
  return listen<CurrentTimeline | null>(CURRENT_TIMELINE_CHANGED_EVENT, (event) => {
    handleTimeline(event.payload)
  })
}

/** 订阅不包含封面的轻量播放状态变化。 */
export function listenToCurrentPlaybackStateChanges(
  handlePlaybackState: (state: CurrentPlaybackState) => void,
): Promise<UnlistenFn> {
  return listen<CurrentPlaybackState>(CURRENT_PLAYBACK_STATE_CHANGED_EVENT, (event) => {
    handlePlaybackState(event.payload)
  })
}

/** 订阅当前媒体统一快照，供 Store 使用单一事件更新状态。 */
export function listenToCurrentMediaSnapshotChanges(
  handleSnapshot: (snapshot: MediaSnapshot | null) => void,
): Promise<UnlistenFn> {
  return listen<MediaSnapshot | null>(CURRENT_MEDIA_SNAPSHOT_CHANGED_EVENT, (event) => {
    handleSnapshot(event.payload)
  })
}
