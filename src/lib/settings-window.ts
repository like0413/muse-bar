import { invoke } from '@tauri-apps/api/core'

let settingsWindowRequest: Promise<void> | undefined

/**
 * 请求 Rust 打开唯一设置窗口，并合并短时间内的重复请求。
 *
 * 托盘、第二次启动和 Bar 右键都复用同一 Rust 流程，因此窗口尺寸和生命周期
 * 不会在多个调用入口中重复定义。
 */
export function openSettingsWindow(): Promise<void> {
  if (!settingsWindowRequest) {
    settingsWindowRequest = invoke<void>('open_settings_window').finally(() => {
      settingsWindowRequest = undefined
    })
  }

  return settingsWindowRequest
}

/** 设置页完成数据读取和首次渲染后，请求显示原生窗口。 */
export function showReadySettingsWindow(): Promise<void> {
  return invoke<void>('show_ready_settings_window')
}
