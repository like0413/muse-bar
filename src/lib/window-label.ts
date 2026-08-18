import { getCurrentWindow } from '@tauri-apps/api/window'

export function readCurrentWindowLabel(): string {
  try {
    return getCurrentWindow().label
  } catch {
    return 'browser-preview'
  }
}
