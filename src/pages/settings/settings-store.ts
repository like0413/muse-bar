import type { UnlistenFn } from '@tauri-apps/api/event'
import { defineStore } from 'pinia'
import { shallowRef } from 'vue'

import {
  getCurrentMediaSnapshot,
  getMediaSessionActivities,
  getMediaSessionIdentities,
  listenToCurrentMediaSnapshotChanges,
  listenToCurrentTimelineChanges,
  listenToMediaSessionActivityChanges,
  listenToMediaSessionIdentityChanges,
  type MediaSessionActivity,
  type MediaSessionIdentity,
  type MediaSnapshot,
} from '@/lib/media-api'
import { getRuntimeInfo } from '@/lib/runtime-info'
import {
  getSettings,
  updateSettings,
  type SettingsPatch,
  type SettingsPayload,
} from '@/lib/settings-api'
import {
  getTaskbarDpi,
  getTaskbarIdentity,
  getTaskbarOccupiedRegions,
  getWindowsVersion,
  openLogDirectory,
  type TaskbarDpi,
  type TaskbarIdentity,
  type TaskbarOccupancy,
  type WindowsVersion,
} from '@/lib/taskbar-diagnostics-api'
import { getTaskbarMonitors, type TaskbarMonitor } from '@/lib/taskbar-monitor-api'
import { readCurrentWindowLabel } from '@/lib/window-label'
import type { RuntimeInfo } from '@/types/runtime-info'

export const useSettingsStore = defineStore('settings', () => {
  const runtimeInfo = shallowRef<RuntimeInfo>()
  const runtimeError = shallowRef('')
  const windowsVersion = shallowRef<WindowsVersion>()
  const settings = shallowRef<SettingsPayload>()
  const settingsError = shallowRef('')
  const isSavingSettings = shallowRef(false)
  const taskbarIdentity = shallowRef<TaskbarIdentity>()
  const taskbarDpi = shallowRef<TaskbarDpi>()
  const taskbarOccupancy = shallowRef<TaskbarOccupancy>()
  const taskbarDiagnosticError = shallowRef('')
  const taskbarMonitorError = shallowRef('')
  const taskbarMonitors = shallowRef<TaskbarMonitor[]>([])
  const mediaSnapshot = shallowRef<MediaSnapshot | null>(null)
  const mediaSnapshotError = shallowRef('')
  const mediaSessionIdentities = shallowRef<MediaSessionIdentity[]>([])
  const mediaSessionActivities = shallowRef<MediaSessionActivity[]>([])
  const logDirectoryError = shallowRef('')
  const windowLabel = readCurrentWindowLabel()
  const listeners: UnlistenFn[] = []
  let isActive = false

  /** 注册异步创建的监听器；页面提前销毁时立即解除监听。 */
  async function registerListener(listenerPromise: Promise<UnlistenFn>): Promise<void> {
    const stopListener = await listenerPromise
    if (!isActive) {
      stopListener()
      return
    }
    listeners.push(stopListener)
  }

  /** 保存局部设置补丁，并采用 Rust 返回的规范化完整设置。 */
  async function saveSettingsPatch(patch: SettingsPatch): Promise<void> {
    if (!settings.value || isSavingSettings.value) return
    isSavingSettings.value = true
    settingsError.value = ''
    try {
      settings.value = await updateSettings({ ...settings.value, ...patch })
    } catch (error) {
      settingsError.value = error instanceof Error ? error.message : String(error)
    } finally {
      isSavingSettings.value = false
    }
  }

  /** 并行读取应用启动信息和 Windows 构建号，单项失败不遮住另一项。 */
  async function loadRuntimeInfo(): Promise<void> {
    const [runtimeResult, windowsResult] = await Promise.allSettled([
      getRuntimeInfo(),
      getWindowsVersion(),
    ])
    if (!isActive) return
    if (runtimeResult.status === 'fulfilled') runtimeInfo.value = runtimeResult.value
    if (windowsResult.status === 'fulfilled') windowsVersion.value = windowsResult.value

    const errors: string[] = []
    if (runtimeResult.status === 'rejected') errors.push(String(runtimeResult.reason))
    if (windowsResult.status === 'rejected') errors.push(String(windowsResult.reason))
    runtimeError.value = errors.join('；')
  }

  /** 读取 Rust 持有的完整设置。 */
  async function loadSettings(): Promise<void> {
    try {
      const currentSettings = await getSettings()
      if (!isActive) return
      settings.value = currentSettings
      settingsError.value = ''
    } catch (error) {
      if (isActive) settingsError.value = error instanceof Error ? error.message : String(error)
    }
  }

  /** 读取当前具有任务栏的显示器列表。 */
  async function loadTaskbarMonitors(): Promise<void> {
    try {
      const monitors = await getTaskbarMonitors()
      if (!isActive) return
      taskbarMonitors.value = monitors
      taskbarMonitorError.value = ''
    } catch (error) {
      if (isActive)
        taskbarMonitorError.value = error instanceof Error ? error.message : String(error)
    }
  }

  /** 并行刷新任务栏身份、DPI 和占用区域。 */
  async function loadTaskbarDiagnostics(): Promise<void> {
    taskbarDiagnosticError.value = ''
    try {
      const [identity, dpi, occupancy] = await Promise.all([
        getTaskbarIdentity(),
        getTaskbarDpi(),
        getTaskbarOccupiedRegions(),
      ])
      if (!isActive) return
      taskbarIdentity.value = identity
      taskbarDpi.value = dpi
      taskbarOccupancy.value = occupancy
    } catch (error) {
      if (isActive)
        taskbarDiagnosticError.value = error instanceof Error ? error.message : String(error)
    }
  }

  /** 打开日志目录，并把失败原因留给诊断分区展示。 */
  async function openLogs(): Promise<void> {
    logDirectoryError.value = ''
    try {
      await openLogDirectory()
    } catch (error) {
      logDirectoryError.value = error instanceof Error ? error.message : String(error)
    }
  }

  /** 监听统一媒体快照和轻量时间轴，并读取首次快照。 */
  async function startMediaSnapshotListener(): Promise<void> {
    try {
      await registerListener(
        listenToCurrentMediaSnapshotChanges((snapshot) => {
          mediaSnapshot.value = snapshot
          mediaSnapshotError.value = ''
        }),
      )
      await registerListener(
        listenToCurrentTimelineChanges((timeline) => {
          if (mediaSnapshot.value) mediaSnapshot.value = { ...mediaSnapshot.value, timeline }
        }),
      )
      const snapshot = await getCurrentMediaSnapshot()
      if (isActive) mediaSnapshot.value = snapshot
    } catch (error) {
      if (isActive)
        mediaSnapshotError.value = error instanceof Error ? error.message : String(error)
    }
  }

  /** 监听播放器会话列表，并读取页面打开前已存在的会话。 */
  async function startMediaSessionIdentityListener(): Promise<void> {
    try {
      await registerListener(
        listenToMediaSessionIdentityChanges((identities) => {
          mediaSessionIdentities.value = identities
        }),
      )
      const identities = await getMediaSessionIdentities()
      if (isActive) mediaSessionIdentities.value = identities
    } catch (error) {
      if (isActive)
        mediaSnapshotError.value = error instanceof Error ? error.message : String(error)
    }
  }

  /** 监听播放器活动记录，并读取已有记录。 */
  async function startMediaSessionActivityListener(): Promise<void> {
    try {
      await registerListener(
        listenToMediaSessionActivityChanges((activities) => {
          mediaSessionActivities.value = activities
        }),
      )
      const activities = await getMediaSessionActivities()
      if (isActive) mediaSessionActivities.value = activities
    } catch (error) {
      if (isActive)
        mediaSnapshotError.value = error instanceof Error ? error.message : String(error)
    }
  }

  /** 启动设置页唯一的一组数据读取与事件监听。 */
  async function start(): Promise<void> {
    if (isActive) return
    isActive = true
    await Promise.all([
      loadRuntimeInfo(),
      loadSettings(),
      loadTaskbarMonitors(),
      loadTaskbarDiagnostics(),
      startMediaSnapshotListener(),
      startMediaSessionIdentityListener(),
      startMediaSessionActivityListener(),
    ])
  }

  /** 解除设置页建立的全部事件监听。 */
  function stop(): void {
    isActive = false
    for (const stopListener of listeners.splice(0)) stopListener()
  }

  return {
    runtimeInfo,
    runtimeError,
    windowsVersion,
    settings,
    settingsError,
    isSavingSettings,
    taskbarIdentity,
    taskbarDpi,
    taskbarOccupancy,
    taskbarDiagnosticError,
    taskbarMonitorError,
    taskbarMonitors,
    mediaSnapshot,
    mediaSnapshotError,
    mediaSessionIdentities,
    mediaSessionActivities,
    logDirectoryError,
    windowLabel,
    saveSettingsPatch,
    loadTaskbarDiagnostics,
    openLogs,
    start,
    stop,
  }
})
