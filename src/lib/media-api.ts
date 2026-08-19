import { invoke } from '@tauri-apps/api/core'

/** 查询 Rust 进程是否已经取得 Windows 全局系统媒体管理器。 */
export function isSystemMediaManagerInitialized(): Promise<boolean> {
  return invoke<boolean>('is_system_media_manager_initialized')
}

/** 读取当前所有 Windows 系统媒体会话的 Source App ID。 */
export function getMediaSessionSourceAppIds(): Promise<string[]> {
  return invoke<string[]>('get_media_session_source_app_ids')
}
