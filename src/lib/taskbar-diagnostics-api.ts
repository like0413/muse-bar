import { invoke } from '@tauri-apps/api/core'

export type TaskbarOccupancySource = 'uiAutomation' | 'win32Fallback'

export interface TaskbarOccupiedRegion {
  name: string
  className: string
  left: number
  top: number
  right: number
  bottom: number
  width: number
  height: number
}

export interface TaskbarOccupancy {
  source: TaskbarOccupancySource
  fallbackReason: string | null
  regions: TaskbarOccupiedRegion[]
}

/** 请求 Rust 读取主任务栏当前可见的系统控件占用区域。 */
export function getTaskbarOccupiedRegions(): Promise<TaskbarOccupancy> {
  return invoke<TaskbarOccupancy>('get_taskbar_occupied_regions')
}
