import { invoke } from '@tauri-apps/api/core'

import type { MediaSessionActivity, MediaSessionIdentity } from './media-types'

/** 读取全部媒体会话的原始来源标识和 Muse Bar 播放器分类。 */
export function getMediaSessionIdentities(): Promise<MediaSessionIdentity[]> {
  return invoke<MediaSessionIdentity[]>('get_media_session_identities')
}

/** 读取全部媒体会话最近一次有效活动的诊断状态。 */
export function getMediaSessionActivities(): Promise<MediaSessionActivity[]> {
  return invoke<MediaSessionActivity[]>('get_media_session_activities')
}
