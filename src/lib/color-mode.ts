import type { UnlistenFn } from '@tauri-apps/api/event'

import {
  getSettings,
  listenToSettingsChanges,
  readColorMode,
  type ColorMode,
  type SettingsPayload,
} from '@/lib/settings-api'

/** 根据用户设置和系统偏好，给当前 WebView 的根节点应用深色类名。 */
function applyColorMode(colorMode: ColorMode, systemColorMode: MediaQueryList): void {
  const useDark = colorMode === 'dark' || (colorMode === 'system' && systemColorMode.matches)
  document.documentElement.classList.toggle('dark', useDark)
}

/** 让当前前端窗口持续跟随持久化设置，并在系统模式变化时即时刷新。 */
export async function startColorModeSync(): Promise<() => void> {
  const systemColorMode = window.matchMedia('(prefers-color-scheme: dark)')
  let currentColorMode: ColorMode = 'system'
  let stopSettingsListener: UnlistenFn | undefined

  /** 将当前内存状态重新应用到当前 WebView。 */
  const applyCurrentMode = () => applyColorMode(currentColorMode, systemColorMode)
  /** 接收 Rust 广播的完整设置并提取颜色模式。 */
  const handleSettings = (settings: SettingsPayload) => {
    currentColorMode = readColorMode(settings)
    applyCurrentMode()
  }
  /** 仅在跟随系统时响应 Windows 颜色模式变化。 */
  const handleSystemColorModeChange = () => {
    if (currentColorMode === 'system') applyCurrentMode()
  }

  // 异步读取设置完成前先使用系统模式，避免窗口初次出现时固定闪成浅色。
  applyCurrentMode()
  systemColorMode.addEventListener('change', handleSystemColorModeChange)

  try {
    // 先订阅再读取，避免初始化期间恰好发生的设置更新被遗漏。
    stopSettingsListener = await listenToSettingsChanges(handleSettings)
    handleSettings(await getSettings())
  } catch {
    // 设置通路暂时不可用时保留“跟随系统”，窗口仍可正常显示。
  }

  return () => {
    stopSettingsListener?.()
    systemColorMode.removeEventListener('change', handleSystemColorModeChange)
  }
}
