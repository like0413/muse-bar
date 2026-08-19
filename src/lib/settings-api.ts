import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type SettingsPayload = Record<string, unknown>

const SETTINGS_CHANGED_EVENT = 'settings-changed'

/** 从 Rust 全局状态读取当前完整设置。 */
export function getSettings(): Promise<SettingsPayload> {
  return invoke<SettingsPayload>('get_settings')
}

/** 将完整设置交给 Rust 持久化，并返回实际保存的结果。 */
export function updateSettings(settings: SettingsPayload): Promise<SettingsPayload> {
  return invoke<SettingsPayload>('update_settings', { settings })
}

/** 订阅 Rust 广播的设置变化，并返回用于销毁监听器的函数。 */
export function listenToSettingsChanges(
  handleSettings: (settings: SettingsPayload) => void,
): Promise<UnlistenFn> {
  return listen<SettingsPayload>(SETTINGS_CHANGED_EVENT, (event) => {
    handleSettings(event.payload)
  })
}

/** 从尚未生成静态类型的设置载荷中安全读取任务栏位置。 */
export function readTaskbarPosition(settings: SettingsPayload | undefined): string | undefined {
  return typeof settings?.position === 'string' ? settings.position : undefined
}
