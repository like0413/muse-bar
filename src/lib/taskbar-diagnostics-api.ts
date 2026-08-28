import { invoke } from '@tauri-apps/api/core'

export type TaskbarOccupancySource = 'uiAutomation' | 'win32Fallback'

export interface TaskbarOccupancy {
  source: TaskbarOccupancySource
  fallbackReason: string | null
  regionCount: number
}

export interface TaskbarIdentity {
  hwnd: number
  explorerProcessId: number
}

export interface TaskbarDpi {
  dpi: number
  scaleFactor: number
  physicalWidth: number
  physicalHeight: number
}

export interface WindowsVersion {
  productName: string
  version: string
  build: number
}

/** 请求 Rust 读取真实的 Windows 版本号和构建号。 */
export function getWindowsVersion(): Promise<WindowsVersion> {
  return invoke<WindowsVersion>('get_windows_version')
}

/** 请求 Rust 查找并验证主任务栏身份。 */
export function getTaskbarIdentity(): Promise<TaskbarIdentity> {
  return invoke<TaskbarIdentity>('get_taskbar_identity')
}

/** 请求 Rust 读取任务栏窗口自身的 DPI 与尺寸换算结果。 */
export function getTaskbarDpi(): Promise<TaskbarDpi> {
  return invoke<TaskbarDpi>('get_taskbar_dpi')
}

/** 请求 Rust 读取主任务栏当前可见的系统控件占用区域。 */
export function getTaskbarOccupiedRegions(): Promise<TaskbarOccupancy> {
  return invoke<TaskbarOccupancy>('get_taskbar_occupied_regions')
}

/** 让 Rust 创建并使用资源管理器打开应用日志目录。 */
export function openLogDirectory(): Promise<void> {
  return invoke<void>('open_log_directory')
}
