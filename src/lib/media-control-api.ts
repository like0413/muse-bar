import { invoke } from '@tauri-apps/api/core'

import type { ControlAction } from './media-types'

/** 对 Rust 当前选中的媒体会话执行一个控制动作。 */
export function controlMedia(action: ControlAction): Promise<void> {
  return invoke<void>('control_media', { action })
}
