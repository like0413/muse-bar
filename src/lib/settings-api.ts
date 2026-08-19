import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type SettingsPayload = Record<string, unknown>
export type ColorMode = 'system' | 'dark' | 'light'
export type ProgressStyle = 'underline' | 'background-gradient'

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

/** 从设置载荷中读取颜色模式，旧设置缺少该字段时默认跟随系统。 */
export function readColorMode(settings: SettingsPayload | undefined): ColorMode {
  const colorMode = settings?.colorMode
  return colorMode === 'dark' || colorMode === 'light' ? colorMode : 'system'
}

/** 从设置载荷中读取进度样式，缺少或无法识别时使用默认的底部细线。 */
export function readProgressStyle(settings: SettingsPayload | undefined): ProgressStyle {
  return settings?.progressStyle === 'background-gradient' ? 'background-gradient' : 'underline'
}
