import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

const MEDIA_SESSIONS_CHANGED_EVENT = 'media-sessions-changed'
const CURRENT_MEDIA_METADATA_CHANGED_EVENT = 'current-media-metadata-changed'
const CURRENT_PLAYBACK_STATUS_CHANGED_EVENT = 'current-playback-status-changed'
const CURRENT_PLAYBACK_CAPABILITIES_CHANGED_EVENT = 'current-playback-capabilities-changed'
const CURRENT_TIMELINE_CHANGED_EVENT = 'current-timeline-changed'

export type CurrentPlaybackStatus =
  | 'closed'
  | 'opened'
  | 'changing'
  | 'stopped'
  | 'playing'
  | 'paused'
  | 'unknown'

export interface CurrentMediaMetadata {
  sourceAppId: string
  title: string
  artist: string
  artworkDataUrl: string | null
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

/** 查询 Rust 进程是否已经取得 Windows 全局系统媒体管理器。 */
export function isSystemMediaManagerInitialized(): Promise<boolean> {
  return invoke<boolean>('is_system_media_manager_initialized')
}

/** 读取当前所有 Windows 系统媒体会话的 Source App ID。 */
export function getMediaSessionSourceAppIds(): Promise<string[]> {
  return invoke<string[]>('get_media_session_source_app_ids')
}

/** 订阅系统媒体会话列表变化，并返回用于取消订阅的函数。 */
export function listenToMediaSessionChanges(
  handleSourceAppIds: (sourceAppIds: string[]) => void,
): Promise<UnlistenFn> {
  return listen<string[]>(MEDIA_SESSIONS_CHANGED_EVENT, (event) => {
    handleSourceAppIds(event.payload)
  })
}

/** 从 Rust 缓存读取 Windows 当前媒体会话的标题、歌手和封面。 */
export function getCurrentMediaMetadata(): Promise<CurrentMediaMetadata | null> {
  return invoke<CurrentMediaMetadata | null>('get_current_media_metadata')
}

/** 订阅 Windows 当前媒体会话的完整元数据变化。 */
export function listenToCurrentMediaMetadataChanges(
  handleMetadata: (metadata: CurrentMediaMetadata | null) => void,
): Promise<UnlistenFn> {
  return listen<CurrentMediaMetadata | null>(CURRENT_MEDIA_METADATA_CHANGED_EVENT, (event) => {
    handleMetadata(event.payload)
  })
}

/** 读取 Windows 当前媒体会话的播放状态。 */
export function getCurrentPlaybackStatus(): Promise<CurrentPlaybackStatus | null> {
  return invoke<CurrentPlaybackStatus | null>('get_current_playback_status')
}

/** 订阅 Windows 当前媒体会话的播放状态变化。 */
export function listenToCurrentPlaybackStatusChanges(
  handleStatus: (status: CurrentPlaybackStatus | null) => void,
): Promise<UnlistenFn> {
  return listen<CurrentPlaybackStatus | null>(CURRENT_PLAYBACK_STATUS_CHANGED_EVENT, (event) => {
    handleStatus(event.payload)
  })
}

/** 读取 Windows 当前媒体会话声明的控制能力。 */
export function getCurrentPlaybackCapabilities(): Promise<CurrentPlaybackCapabilities | null> {
  return invoke<CurrentPlaybackCapabilities | null>('get_current_playback_capabilities')
}

/** 订阅 Windows 当前媒体会话控制能力变化。 */
export function listenToCurrentPlaybackCapabilitiesChanges(
  handleCapabilities: (capabilities: CurrentPlaybackCapabilities | null) => void,
): Promise<UnlistenFn> {
  return listen<CurrentPlaybackCapabilities | null>(
    CURRENT_PLAYBACK_CAPABILITIES_CHANGED_EVENT,
    (event) => {
      handleCapabilities(event.payload)
    },
  )
}

/** 读取 Windows 当前媒体会话上报的有效时间轴。 */
export function getCurrentTimeline(): Promise<CurrentTimeline | null> {
  return invoke<CurrentTimeline | null>('get_current_timeline')
}

/** 订阅 Windows 当前媒体会话时间轴变化。 */
export function listenToCurrentTimelineChanges(
  handleTimeline: (timeline: CurrentTimeline | null) => void,
): Promise<UnlistenFn> {
  return listen<CurrentTimeline | null>(CURRENT_TIMELINE_CHANGED_EVENT, (event) => {
    handleTimeline(event.payload)
  })
}
