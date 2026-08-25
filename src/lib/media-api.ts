import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

const MEDIA_SESSION_IDENTITIES_CHANGED_EVENT = 'media-session-identities-changed'
const MEDIA_SESSION_ACTIVITIES_CHANGED_EVENT = 'media-session-activities-changed'
const CURRENT_TIMELINE_CHANGED_EVENT = 'current-timeline-changed'
const CURRENT_MEDIA_SNAPSHOT_CHANGED_EVENT = 'current-media-snapshot-changed'

export type CurrentPlaybackStatus =
  | 'closed'
  | 'opened'
  | 'changing'
  | 'stopped'
  | 'playing'
  | 'paused'
  | 'unknown'

export type MediaPlayerKind =
  | 'qqMusic'
  | 'neteaseCloudMusic'
  | 'kugouMusic'
  | 'qishuiMusic'
  | 'other'

export interface MediaSessionIdentity {
  sessionKey: number
  sourceAppId: string
  playerKind: MediaPlayerKind
}

export type MediaActivityReason =
  | 'detectedPlaying'
  | 'playbackStarted'
  | 'trackChanged'
  | 'becameCurrent'

export interface MediaSessionActivity {
  sessionKey: number
  sourceAppId: string
  playerKind: MediaPlayerKind
  title: string | null
  artist: string | null
  isPlaying: boolean
  isPaused: boolean
  lastActivityAtUnixMs: number | null
  activitySequence: number | null
  lastActivityReason: MediaActivityReason | null
}

export type MediaSelectionReason =
  | 'playingPreferred'
  | 'lastPausedPreferred'
  | 'detectedPreferred'
  | 'windowsCurrentFallback'

export interface SelectedMediaSession {
  sessionKey: number
  sourceAppId: string
  playerKind: MediaPlayerKind
  reason: MediaSelectionReason
}

export type ControlAction =
  | { type: 'togglePlayPause' }
  | { type: 'previous' }
  | { type: 'next' }
  | { type: 'seek'; positionMs: number }

export type MediaControlErrorCode = 'noSession' | 'unsupported' | 'rejected' | 'windowsApi'

export interface MediaControlError {
  action: ControlAction
  code: MediaControlErrorCode
  message: string
}

export interface CurrentMediaMetadata {
  sourceAppId: string
  title: string
  artist: string
  artworkDataUrl: string | null
  accentColor: string
}

export interface CurrentPlaybackCapabilities {
  canPlay: boolean
  canPause: boolean
  canPrevious: boolean
  canNext: boolean
  canSeek: boolean
}

export interface CurrentTimeline {
  startMs: number
  endMs: number
  positionMs: number
  minSeekMs: number
  maxSeekMs: number
  lastUpdatedAtUnixMs: number
  playbackRate: number | null
}

export interface MediaSnapshot {
  sessionKey: number
  sourceAppId: string
  playerKind: MediaPlayerKind
  title: string
  artist: string
  artworkDataUrl: string | null
  accentColor: string
  systemAccentColor: string
  playbackStatus: CurrentPlaybackStatus
  capabilities: CurrentPlaybackCapabilities
  timeline: CurrentTimeline | null
}

/** 读取全部媒体会话的原始来源标识和 Muse Bar 播放器分类。 */
export function getMediaSessionIdentities(): Promise<MediaSessionIdentity[]> {
  return invoke<MediaSessionIdentity[]>('get_media_session_identities')
}

/** 订阅媒体会话身份变化，并返回用于取消订阅的函数。 */
export function listenToMediaSessionIdentityChanges(
  handleIdentities: (identities: MediaSessionIdentity[]) => void,
): Promise<UnlistenFn> {
  return listen<MediaSessionIdentity[]>(MEDIA_SESSION_IDENTITIES_CHANGED_EVENT, (event) => {
    handleIdentities(event.payload)
  })
}

/** 读取全部媒体会话最近一次有效活动的诊断状态。 */
export function getMediaSessionActivities(): Promise<MediaSessionActivity[]> {
  return invoke<MediaSessionActivity[]>('get_media_session_activities')
}

/** 订阅播放开始、切歌或 CurrentSession 变化产生的活动记录。 */
export function listenToMediaSessionActivityChanges(
  handleActivities: (activities: MediaSessionActivity[]) => void,
): Promise<UnlistenFn> {
  return listen<MediaSessionActivity[]>(MEDIA_SESSION_ACTIVITIES_CHANGED_EVENT, (event) => {
    handleActivities(event.payload)
  })
}

/** 要求 Rust 按活动记录重新选择 Bar 实际观察的媒体会话。 */
export function refreshSelectedMediaSession(): Promise<SelectedMediaSession | null> {
  return invoke<SelectedMediaSession | null>('refresh_selected_media_session')
}

/** 对 Rust 当前选中的媒体会话执行一个控制动作。 */
export function controlMedia(action: ControlAction): Promise<void> {
  return invoke<void>('control_media', { action })
}

/** 订阅 Windows 当前媒体会话时间轴变化。 */
export function listenToCurrentTimelineChanges(
  handleTimeline: (timeline: CurrentTimeline | null) => void,
): Promise<UnlistenFn> {
  return listen<CurrentTimeline | null>(CURRENT_TIMELINE_CHANGED_EVENT, (event) => {
    handleTimeline(event.payload)
  })
}

/** 从 Rust 读取当前会话各项数据组成的统一媒体快照。 */
export function getCurrentMediaSnapshot(): Promise<MediaSnapshot | null> {
  return invoke<MediaSnapshot | null>('get_current_media_snapshot')
}

/** 订阅当前媒体统一快照，供后续 Store 使用单一事件更新状态。 */
export function listenToCurrentMediaSnapshotChanges(
  handleSnapshot: (snapshot: MediaSnapshot | null) => void,
): Promise<UnlistenFn> {
  return listen<MediaSnapshot | null>(CURRENT_MEDIA_SNAPSHOT_CHANGED_EVENT, (event) => {
    handleSnapshot(event.payload)
  })
}
