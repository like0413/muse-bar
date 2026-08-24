import type { UnlistenFn } from '@tauri-apps/api/event'
import { defineStore } from 'pinia'
import { shallowRef } from 'vue'

import {
  getCurrentMediaSnapshot,
  listenToCurrentMediaSnapshotChanges,
  listenToMediaSessionActivityChanges,
  refreshSelectedMediaSession,
  type MediaSelectionReason,
  type MediaSnapshot,
} from '@/lib/media-api'
import { getSettings, listenToSettingsChanges, type SettingsPayload } from '@/lib/settings-api'

const selectionReasonLabels: Record<MediaSelectionReason, string> = {
  playingPreferred: '最近播放的音乐播放器',
  lastPausedPreferred: '最近暂停的音乐播放器',
  windowsCurrentFallback: 'Windows 当前媒体',
}

export const useBarStore = defineStore('bar', () => {
  const snapshot = shallowRef<MediaSnapshot | null>(null)
  const settings = shallowRef<SettingsPayload>()
  const mediaStatus = shallowRef('正在读取媒体信息')
  const mediaSelectionText = shallowRef('')
  const barWidthDetails = shallowRef('宽度：等待测量')
  const settingsWindowError = shallowRef('')
  const controlError = shallowRef('')
  const listeners: UnlistenFn[] = []
  let isActive = false

  /** 应用 Rust 推送的统一媒体快照，并同步无会话提示。 */
  function applySnapshot(nextSnapshot: MediaSnapshot | null): void {
    snapshot.value = nextSnapshot
    mediaStatus.value = nextSnapshot ? '' : '当前没有媒体会话'
  }

  /** 应用设置页和 Rust 广播的完整设置。 */
  function applySettings(nextSettings: SettingsPayload): void {
    settings.value = nextSettings
  }

  /** 注册异步创建的事件监听器；若页面已卸载则立即销毁它。 */
  async function registerListener(listenerPromise: Promise<UnlistenFn>): Promise<void> {
    const stopListener = await listenerPromise
    if (!isActive) {
      stopListener()
      return
    }
    listeners.push(stopListener)
  }

  /** 按最新播放器活动重新选择会话，并保存供悬停诊断使用的选择原因。 */
  async function refreshMediaSelection(): Promise<void> {
    try {
      const selection = await refreshSelectedMediaSession()
      if (!isActive) return
      mediaSelectionText.value = selection
        ? `选择：${selectionReasonLabels[selection.reason]}`
        : '选择：当前没有媒体会话'
    } catch {
      if (isActive) mediaSelectionText.value = '选择：刷新失败'
    }
  }

  /** 建立统一媒体快照监听，并在监听就绪后读取一次当前值。 */
  async function startSnapshotListener(): Promise<void> {
    try {
      await registerListener(listenToCurrentMediaSnapshotChanges(applySnapshot))
      const currentSnapshot = await getCurrentMediaSnapshot()
      if (isActive) applySnapshot(currentSnapshot)
    } catch {
      if (isActive) mediaStatus.value = '媒体信息监听失败'
    }
  }

  /** 监听播放器有效活动，仅在需要时要求 Rust 重新选择当前会话。 */
  async function startSelectionListener(): Promise<void> {
    try {
      await registerListener(
        listenToMediaSessionActivityChanges(() => {
          void refreshMediaSelection()
        }),
      )
      await refreshMediaSelection()
    } catch {
      if (isActive) mediaSelectionText.value = '选择：监听失败'
    }
  }

  /** 建立设置监听，并在监听就绪后读取一次持久化设置。 */
  async function startSettingsListener(): Promise<void> {
    try {
      await registerListener(listenToSettingsChanges(applySettings))
      const currentSettings = await getSettings()
      if (isActive) applySettings(currentSettings)
    } catch (error) {
      if (!isActive) return
      barWidthDetails.value = `设置监听失败：${error instanceof Error ? error.message : String(error)}`
    }
  }

  /** 启动 Bar 页面唯一的一组全局事件监听，防止子组件重复订阅。 */
  async function start(): Promise<void> {
    if (isActive) return
    isActive = true
    await Promise.all([startSnapshotListener(), startSelectionListener(), startSettingsListener()])
  }

  /** 销毁 Bar 页面建立的全部监听器。 */
  function stop(): void {
    isActive = false
    for (const stopListener of listeners.splice(0)) stopListener()
  }

  function setBarWidthDetails(details: string): void {
    barWidthDetails.value = details
  }

  function setSettingsWindowError(message: string): void {
    settingsWindowError.value = message
  }

  function setControlError(message: string): void {
    controlError.value = message
  }

  return {
    snapshot,
    settings,
    mediaStatus,
    mediaSelectionText,
    barWidthDetails,
    settingsWindowError,
    controlError,
    start,
    stop,
    setBarWidthDetails,
    setSettingsWindowError,
    setControlError,
  }
})
