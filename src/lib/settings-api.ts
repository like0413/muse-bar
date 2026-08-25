import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type SettingsPayload = Record<string, unknown>
export type ColorMode = 'system' | 'dark' | 'light'
export type ControlPosition = 'left' | 'right'
export type ProgressColorSource = 'artwork' | 'system' | 'custom'
export type ProgressStyle = 'underline' | 'background-gradient'
export type TaskbarPosition = 'left' | 'center' | 'right'
export type TitleScrollMode = 'continuous' | 'restart' | 'bounce'
export type WindowMode = 'auto' | 'owner'

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

/** 从设置载荷中读取任务栏位置，异常值回退到产品默认的靠右。 */
export function readTaskbarPosition(settings: SettingsPayload | undefined): TaskbarPosition {
  const position = settings?.position
  return position === 'left' || position === 'center' ? position : 'right'
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

/** 从设置载荷中读取进度显示开关，旧设置缺少字段时默认显示。 */
export function readShowProgress(settings: SettingsPayload | undefined): boolean {
  return settings?.showProgress !== false
}

/** 从设置载荷中读取进度颜色来源，异常值回退到封面主色。 */
export function readProgressColorSource(
  settings: SettingsPayload | undefined,
): ProgressColorSource {
  const source = settings?.progressColorSource
  return source === 'system' || source === 'custom' ? source : 'artwork'
}

/** 从设置载荷中读取已校验的自定义进度颜色。 */
export function readCustomProgressColor(settings: SettingsPayload | undefined): string {
  const color = settings?.customProgressColor
  return typeof color === 'string' && /^#[0-9a-f]{6}$/i.test(color) ? color : '#0078D4'
}

/** 从设置载荷中读取控制按钮显隐开关，旧设置缺少字段时默认显示。 */
export function readShowControls(settings: SettingsPayload | undefined): boolean {
  return settings?.showControls !== false
}

/** 从设置载荷中读取控制按钮位置，异常值回退到 Bar 右侧。 */
export function readControlPosition(settings: SettingsPayload | undefined): ControlPosition {
  return settings?.controlPosition === 'left' ? 'left' : 'right'
}

/** 从设置载荷中读取标题滚动开关，旧设置缺少字段时默认开启。 */
export function readTitleScrollEnabled(settings: SettingsPayload | undefined): boolean {
  return settings?.titleScrollEnabled !== false
}

/** 从设置载荷中读取标题滚动速度，异常值回退到每秒 30 像素。 */
export function readTitleScrollSpeed(settings: SettingsPayload | undefined): number {
  const speed = settings?.titleScrollSpeed
  return typeof speed === 'number' && Number.isFinite(speed) ? speed : 30
}

/** 从设置载荷中读取标题滚动方式，旧设置缺少字段时默认连续滚动。 */
export function readTitleScrollMode(settings: SettingsPayload | undefined): TitleScrollMode {
  const mode = settings?.titleScrollMode
  return mode === 'restart' || mode === 'bounce' ? mode : 'continuous'
}

/** 从设置载荷中读取 Bar 的最小逻辑宽度。 */
export function readMinimumWidth(settings: SettingsPayload | undefined): number | undefined {
  return typeof settings?.minWidth === 'number' ? settings.minWidth : undefined
}

/** 从设置载荷中读取 Bar 的最大逻辑宽度。 */
export function readMaximumWidth(settings: SettingsPayload | undefined): number | undefined {
  return typeof settings?.maxWidth === 'number' ? settings.maxWidth : undefined
}

/** 从设置载荷中读取 Bar 的水平逻辑像素偏移。 */
export function readManualOffset(settings: SettingsPayload | undefined): number | undefined {
  return typeof settings?.manualOffset === 'number' ? settings.manualOffset : undefined
}

/** 从设置载荷中读取目标显示器设备标识，旧设置默认使用主显示器。 */
export function readTargetMonitor(settings: SettingsPayload | undefined): string {
  const monitor = settings?.targetMonitor
  return typeof monitor === 'string' && monitor.trim() ? monitor : 'primary'
}

/** 从设置载荷中读取窗口宿主模式。 */
export function readWindowMode(settings: SettingsPayload | undefined): WindowMode {
  return settings?.windowMode === 'owner' ? 'owner' : 'auto'
}

/** 从设置载荷中读取开机启动开关。 */
export function readLaunchOnStartup(settings: SettingsPayload | undefined): boolean {
  return settings?.launchOnStartup === true
}
