import { invoke } from '@tauri-apps/api/core'

import type { MediaSelectionReason, MediaSnapshot } from './media-types'

/** 从 Rust 读取当前会话各项数据组成的统一媒体快照。 */
export function getCurrentMediaSnapshot(): Promise<MediaSnapshot | null> {
  return invoke<MediaSnapshot | null>('get_current_media_snapshot')
}

/** 要求 Rust 按活动记录重新选择 Bar 实际观察的媒体会话。 */
export function refreshSelectedMediaSession(): Promise<MediaSelectionReason | null> {
  return invoke<MediaSelectionReason | null>('refresh_selected_media_session')
}
