import { getCurrentWindow } from '@tauri-apps/api/window'

/** 返回当前 Tauri 窗口标签；浏览器预览时使用可读的回退值。 */
export function readCurrentWindowLabel(): string {
  try {
    return getCurrentWindow().label
  } catch {
    return 'browser-preview'
  }
}
