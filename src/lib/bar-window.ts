import { invoke } from '@tauri-apps/api/core'

/** 通知 Rust 当前是否存在可展示媒体，由进程级状态决定 Bar 最终显隐。 */
export function setBarMediaAvailable(available: boolean): Promise<void> {
  return invoke<void>('set_bar_media_available', { available })
}
