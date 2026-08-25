import { invoke } from '@tauri-apps/api/core'

export interface TaskbarMonitor {
  id: string
  label: string
  isPrimary: boolean
}

/** 读取当前具有 Windows 任务栏的显示器列表。 */
export function getTaskbarMonitors(): Promise<TaskbarMonitor[]> {
  return invoke<TaskbarMonitor[]>('get_taskbar_monitors')
}
