import { defineStore } from 'pinia'
import { shallowRef } from 'vue'

import { getMediaSessionActivities, getMediaSessionIdentities } from '@/lib/media-diagnostics-api'
import {
  listenToCurrentMediaSnapshotChanges,
  listenToCurrentPlaybackStateChanges,
  listenToCurrentTimelineChanges,
  listenToMediaSessionActivityChanges,
  listenToMediaSessionIdentityChanges,
} from '@/lib/media-event-api'
import { getCurrentMediaSnapshot } from '@/lib/media-query-api'
import type { MediaSessionActivity, MediaSessionIdentity, MediaSnapshot } from '@/lib/media-types'
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
import { TauriListenerScope } from '@/lib/tauri-listener-scope'
import { readCurrentWindowLabel } from '@/lib/window-label'
import type { RuntimeInfo } from '@/types/runtime-info'

export const useSettingsStore = defineStore('settings', () => {
  const runtimeInfo = shallowRef<RuntimeInfo>()
  const runtimeError = shallowRef('')
  const windowsVersion = shallowRef<WindowsVersion>()
  const settings = shallowRef<SettingsPayload>()
  const settingsError = shallowRef('')
  const isSavingSettings = shallowRef(false)
  const isRefreshingDiagnostics = shallowRef(false)
  const isOpeningLogDirectory = shallowRef(false)
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
  const listenerScope = new TauriListenerScope()
  let pendingSettingsPatch: SettingsPatch | undefined
  let settingsSaveOperation: Promise<void> | undefined
  let taskbarDiagnosticsRevision = 0
  let mediaSnapshotRevision = 0
  let mediaIdentitiesRevision = 0
  let mediaActivitiesRevision = 0

  /** 保存局部设置补丁，并采用 Rust 返回的规范化完整设置。 */
  function saveSettingsPatch(patch: SettingsPatch): Promise<void> {
    if (!settings.value) return Promise.resolve()
    pendingSettingsPatch = { ...pendingSettingsPatch, ...patch }
    settingsSaveOperation ??= drainSettingsSaveQueue().finally(() => {
      settingsSaveOperation = undefined
    })
    return settingsSaveOperation
  }

  /** 串行保存并合并等待中的设置补丁，避免快速操作被丢弃或返回结果乱序。 */
  async function drainSettingsSaveQueue(): Promise<void> {
    isSavingSettings.value = true
    settingsError.value = ''
    try {
      while (pendingSettingsPatch && settings.value) {
        const patch = pendingSettingsPatch
        pendingSettingsPatch = undefined
        try {
          settings.value = await updateSettings({ ...settings.value, ...patch })
        } catch (error) {
          pendingSettingsPatch = undefined
          settingsError.value = error instanceof Error ? error.message : String(error)
          return
        }
      }
    } finally {
      isSavingSettings.value = false
    }
  }

  /** 并行读取应用启动信息和 Windows 构建号，单项失败不遮住另一项。 */
  async function loadRuntimeInfo(lifecycleRevision: number): Promise<void> {
    const [runtimeResult, windowsResult] = await Promise.allSettled([
      getRuntimeInfo(),
      getWindowsVersion(),
    ])
    if (!listenerScope.isCurrent(lifecycleRevision)) return
    if (runtimeResult.status === 'fulfilled') runtimeInfo.value = runtimeResult.value
    if (windowsResult.status === 'fulfilled') windowsVersion.value = windowsResult.value

    const errors: string[] = []
    if (runtimeResult.status === 'rejected') errors.push(String(runtimeResult.reason))
    if (windowsResult.status === 'rejected') errors.push(String(windowsResult.reason))
    runtimeError.value = errors.join('；')
  }

  /** 读取 Rust 持有的完整设置。 */
  async function loadSettings(lifecycleRevision: number): Promise<void> {
    try {
      const currentSettings = await getSettings()
      if (!listenerScope.isCurrent(lifecycleRevision)) return
      settings.value = currentSettings
      settingsError.value = ''
    } catch (error) {
      if (listenerScope.isCurrent(lifecycleRevision))
        settingsError.value = error instanceof Error ? error.message : String(error)
    }
  }

  /** 读取当前具有任务栏的显示器列表。 */
  async function loadTaskbarMonitors(lifecycleRevision: number): Promise<void> {
    try {
      const monitors = await getTaskbarMonitors()
      if (!listenerScope.isCurrent(lifecycleRevision)) return
      taskbarMonitors.value = monitors
      taskbarMonitorError.value = ''
    } catch (error) {
      if (listenerScope.isCurrent(lifecycleRevision))
        taskbarMonitorError.value = error instanceof Error ? error.message : String(error)
    }
  }

  /** 并行刷新任务栏身份、DPI 和占用区域。 */
  async function loadTaskbarDiagnostics(): Promise<void> {
    const requestRevision = ++taskbarDiagnosticsRevision
    isRefreshingDiagnostics.value = true
    taskbarDiagnosticError.value = ''
    try {
      const [identity, dpi, occupancy] = await Promise.all([
        getTaskbarIdentity(),
        getTaskbarDpi(),
        getTaskbarOccupiedRegions(),
      ])
      if (!listenerScope.isActive || requestRevision !== taskbarDiagnosticsRevision) return
      taskbarIdentity.value = identity
      taskbarDpi.value = dpi
      taskbarOccupancy.value = occupancy
    } catch (error) {
      if (listenerScope.isActive && requestRevision === taskbarDiagnosticsRevision)
        taskbarDiagnosticError.value = error instanceof Error ? error.message : String(error)
    } finally {
      if (requestRevision === taskbarDiagnosticsRevision) isRefreshingDiagnostics.value = false
    }
  }

  /** 打开日志目录，并把失败原因留给诊断分区展示。 */
  async function openLogs(): Promise<void> {
    if (isOpeningLogDirectory.value) return
    isOpeningLogDirectory.value = true
    logDirectoryError.value = ''
    try {
      await openLogDirectory()
    } catch (error) {
      logDirectoryError.value = error instanceof Error ? error.message : String(error)
    } finally {
      isOpeningLogDirectory.value = false
    }
  }

  /** 监听统一媒体快照和轻量时间轴，并读取首次快照。 */
  async function startMediaSnapshotListener(lifecycleRevision: number): Promise<void> {
    try {
      const initialRevision = mediaSnapshotRevision
      await listenerScope.register(
        lifecycleRevision,
        listenToCurrentMediaSnapshotChanges((snapshot) => {
          if (!listenerScope.isCurrent(lifecycleRevision)) return
          mediaSnapshotRevision += 1
          mediaSnapshot.value = snapshot
          mediaSnapshotError.value = ''
        }),
      )
      await listenerScope.register(
        lifecycleRevision,
        listenToCurrentPlaybackStateChanges((state) => {
          if (!listenerScope.isCurrent(lifecycleRevision)) return
          if (!mediaSnapshot.value || mediaSnapshot.value.sessionKey !== state.sessionKey) return
          mediaSnapshotRevision += 1
          mediaSnapshot.value = {
            ...mediaSnapshot.value,
            playbackStatus: state.playbackStatus,
            capabilities: state.capabilities,
          }
        }),
      )
      await listenerScope.register(
        lifecycleRevision,
        listenToCurrentTimelineChanges((timeline) => {
          if (!listenerScope.isCurrent(lifecycleRevision)) return
          if (mediaSnapshot.value) {
            mediaSnapshotRevision += 1
            mediaSnapshot.value = { ...mediaSnapshot.value, timeline }
          }
        }),
      )
      const snapshot = await getCurrentMediaSnapshot()
      if (listenerScope.isCurrent(lifecycleRevision) && mediaSnapshotRevision === initialRevision)
        mediaSnapshot.value = snapshot
    } catch (error) {
      if (listenerScope.isCurrent(lifecycleRevision))
        mediaSnapshotError.value = error instanceof Error ? error.message : String(error)
    }
  }

  /** 监听播放器会话列表，并读取页面打开前已存在的会话。 */
  async function startMediaSessionIdentityListener(lifecycleRevision: number): Promise<void> {
    try {
      const initialRevision = mediaIdentitiesRevision
      await listenerScope.register(
        lifecycleRevision,
        listenToMediaSessionIdentityChanges((identities) => {
          if (!listenerScope.isCurrent(lifecycleRevision)) return
          mediaIdentitiesRevision += 1
          mediaSessionIdentities.value = identities
        }),
      )
      const identities = await getMediaSessionIdentities()
      if (listenerScope.isCurrent(lifecycleRevision) && mediaIdentitiesRevision === initialRevision)
        mediaSessionIdentities.value = identities
    } catch (error) {
      if (listenerScope.isCurrent(lifecycleRevision))
        mediaSnapshotError.value = error instanceof Error ? error.message : String(error)
    }
  }

  /** 监听播放器活动记录，并读取已有记录。 */
  async function startMediaSessionActivityListener(lifecycleRevision: number): Promise<void> {
    try {
      const initialRevision = mediaActivitiesRevision
      await listenerScope.register(
        lifecycleRevision,
        listenToMediaSessionActivityChanges((activities) => {
          if (!listenerScope.isCurrent(lifecycleRevision)) return
          mediaActivitiesRevision += 1
          mediaSessionActivities.value = activities
        }),
      )
      const activities = await getMediaSessionActivities()
      if (listenerScope.isCurrent(lifecycleRevision) && mediaActivitiesRevision === initialRevision)
        mediaSessionActivities.value = activities
    } catch (error) {
      if (listenerScope.isCurrent(lifecycleRevision))
        mediaSnapshotError.value = error instanceof Error ? error.message : String(error)
    }
  }

  /** 启动设置页唯一的一组数据读取与事件监听。 */
  async function start(): Promise<void> {
    if (listenerScope.isActive) return
    const lifecycleRevision = listenerScope.activate()
    await Promise.all([
      loadRuntimeInfo(lifecycleRevision),
      loadSettings(lifecycleRevision),
      loadTaskbarMonitors(lifecycleRevision),
      loadTaskbarDiagnostics(),
      startMediaSnapshotListener(lifecycleRevision),
      startMediaSessionIdentityListener(lifecycleRevision),
      startMediaSessionActivityListener(lifecycleRevision),
    ])
  }

  /** 解除设置页建立的全部事件监听。 */
  function stop(): void {
    listenerScope.deactivate()
    taskbarDiagnosticsRevision += 1
    isRefreshingDiagnostics.value = false
  }

  return {
    runtimeInfo,
    runtimeError,
    windowsVersion,
    settings,
    settingsError,
    isSavingSettings,
    isRefreshingDiagnostics,
    isOpeningLogDirectory,
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
