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

export interface CurrentPlaybackState {
  sessionKey: number
  playbackStatus: CurrentPlaybackStatus
  capabilities: CurrentPlaybackCapabilities
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
