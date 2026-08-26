import { defineStore } from 'pinia'
import { shallowRef } from 'vue'

import { setBarMediaAvailable } from '@/lib/bar-window'
import {
  listenToCurrentMediaSnapshotChanges,
  listenToCurrentPlaybackStateChanges,
  listenToCurrentTimelineChanges,
  listenToMediaSessionActivityChanges,
} from '@/lib/media-event-api'
import { getCurrentMediaSnapshot, refreshSelectedMediaSession } from '@/lib/media-query-api'
import type {
  CurrentPlaybackState,
  CurrentTimeline,
  MediaSelectionReason,
  MediaSnapshot,
} from '@/lib/media-types'
import { getSettings, listenToSettingsChanges, type SettingsPayload } from '@/lib/settings-api'
import { markStartupMilestone } from '@/lib/startup-performance'
import { TauriListenerScope } from '@/lib/tauri-listener-scope'

const selectionReasonLabels: Record<MediaSelectionReason, string> = {
  playingPreferred: '最近播放的音乐播放器',
  lastPausedPreferred: '最近暂停的音乐播放器',
  detectedPreferred: '已检测到的音乐播放器',
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
  const listenerScope = new TauriListenerScope()
  let isWaitingForChangedTrackTimeline = false
  let timelineResetAtUnixMs: number | null = null
  let lastReportedMediaAvailable: boolean | undefined
  let mediaAvailabilityQueue = Promise.resolve()
  let snapshotRevision = 0
  let settingsRevision = 0
  let selectionRequestRevision = 0

  /** 判断两份快照是否属于不同歌曲；切换播放器时即使曲目信息相同也视为变化。 */
  function hasTrackChanged(
    currentSnapshot: MediaSnapshot | null,
    nextSnapshot: MediaSnapshot | null,
  ): boolean {
    if (!currentSnapshot || !nextSnapshot) return false
    return (
      currentSnapshot.sessionKey !== nextSnapshot.sessionKey ||
      currentSnapshot.title !== nextSnapshot.title ||
      currentSnapshot.artist !== nextSnapshot.artist
    )
  }

  /** 串行通知 Rust 媒体可用状态，避免快速关闭和恢复会话时显隐命令乱序。 */
  function reportMediaAvailability(available: boolean): void {
    if (lastReportedMediaAvailable === available) return
    lastReportedMediaAvailable = available

    mediaAvailabilityQueue = mediaAvailabilityQueue
      .then(() => setBarMediaAvailable(available))
      .catch((error: unknown) => {
        if (lastReportedMediaAvailable === available) lastReportedMediaAvailable = undefined
        if (!listenerScope.isActive) return
        barWidthDetails.value = `Bar 显隐同步失败：${error instanceof Error ? error.message : String(error)}`
      })
  }

  /** 应用 Rust 推送的统一媒体快照，并同步无会话提示。 */
  function applySnapshot(nextSnapshot: MediaSnapshot | null): void {
    snapshotRevision += 1
    if (nextSnapshot) markStartupMilestone('first-media-snapshot')
    reportMediaAvailability(nextSnapshot !== null && nextSnapshot.playbackStatus !== 'closed')
    const currentSnapshot = snapshot.value
    const trackChanged = hasTrackChanged(currentSnapshot, nextSnapshot)
    if (!nextSnapshot) {
      isWaitingForChangedTrackTimeline = false
      timelineResetAtUnixMs = null
      snapshot.value = null
    } else if (trackChanged) {
      // 歌曲变化后只接受更新时刻不早于此处的时间轴，避免晚到的旧数据恢复进度。
      isWaitingForChangedTrackTimeline = true
      timelineResetAtUnixMs = Date.now()
      snapshot.value = { ...nextSnapshot, timeline: null }
    } else if (isWaitingForChangedTrackTimeline) {
      // 完整快照无法证明时间轴的歌曲归属，等待期间始终保持归零。
      snapshot.value = { ...nextSnapshot, timeline: null }
    } else {
      snapshot.value = nextSnapshot
    }
    mediaStatus.value = nextSnapshot ? '' : '当前没有媒体会话'
  }

  /** 将轻量时间轴事件合并进当前快照，避免仅因进度变化重新传输封面等数据。 */
  function applyTimeline(timeline: CurrentTimeline | null): void {
    if (!snapshot.value) return
    snapshotRevision += 1
    if (isWaitingForChangedTrackTimeline) {
      if (
        !timeline ||
        timelineResetAtUnixMs === null ||
        timeline.lastUpdatedAtUnixMs < timelineResetAtUnixMs
      ) {
        return
      }
      isWaitingForChangedTrackTimeline = false
      timelineResetAtUnixMs = null
    }
    snapshot.value = { ...snapshot.value, timeline }
  }

  /** 合并轻量播放状态；会话不匹配时等待下一份完整快照。 */
  function applyPlaybackState(state: CurrentPlaybackState): void {
    if (!snapshot.value || snapshot.value.sessionKey !== state.sessionKey) return
    snapshotRevision += 1
    reportMediaAvailability(state.playbackStatus !== 'closed')
    snapshot.value = {
      ...snapshot.value,
      playbackStatus: state.playbackStatus,
      capabilities: state.capabilities,
    }
  }

  /** 应用设置页和 Rust 广播的完整设置。 */
  function applySettings(nextSettings: SettingsPayload): void {
    settingsRevision += 1
    settings.value = nextSettings
  }

  /** 按最新播放器活动重新选择会话，并保存供悬停诊断使用的选择原因。 */
  async function refreshMediaSelection(lifecycleRevision: number): Promise<void> {
    const requestRevision = ++selectionRequestRevision
    try {
      const selection = await refreshSelectedMediaSession()
      if (
        !listenerScope.isCurrent(lifecycleRevision) ||
        requestRevision !== selectionRequestRevision
      )
        return
      mediaSelectionText.value = selection
        ? `选择：${selectionReasonLabels[selection.reason]}`
        : '选择：当前没有媒体会话'
    } catch {
      if (
        listenerScope.isCurrent(lifecycleRevision) &&
        requestRevision === selectionRequestRevision
      )
        mediaSelectionText.value = '选择：刷新失败'
    }
  }

  /** 建立媒体快照和轻量时间轴监听，并在监听就绪后读取一次当前值。 */
  async function startSnapshotListener(lifecycleRevision: number): Promise<void> {
    try {
      const initialRevision = snapshotRevision
      await Promise.all([
        listenerScope.register(
          lifecycleRevision,
          listenToCurrentMediaSnapshotChanges(applySnapshot),
        ),
        listenerScope.register(lifecycleRevision, listenToCurrentTimelineChanges(applyTimeline)),
        listenerScope.register(
          lifecycleRevision,
          listenToCurrentPlaybackStateChanges(applyPlaybackState),
        ),
      ])
      const currentSnapshot = await getCurrentMediaSnapshot()
      if (listenerScope.isCurrent(lifecycleRevision) && snapshotRevision === initialRevision)
        applySnapshot(currentSnapshot)
    } catch {
      if (listenerScope.isCurrent(lifecycleRevision)) mediaStatus.value = '媒体信息监听失败'
    }
  }

  /** 监听播放器有效活动，仅在需要时要求 Rust 重新选择当前会话。 */
  async function startSelectionListener(lifecycleRevision: number): Promise<void> {
    try {
      await listenerScope.register(
        lifecycleRevision,
        listenToMediaSessionActivityChanges(() => {
          void refreshMediaSelection(lifecycleRevision)
        }),
      )
      await refreshMediaSelection(lifecycleRevision)
    } catch {
      if (listenerScope.isCurrent(lifecycleRevision)) mediaSelectionText.value = '选择：监听失败'
    }
  }

  /** 建立设置监听，并在监听就绪后读取一次持久化设置。 */
  async function startSettingsListener(lifecycleRevision: number): Promise<void> {
    try {
      const initialRevision = settingsRevision
      await listenerScope.register(lifecycleRevision, listenToSettingsChanges(applySettings))
      const currentSettings = await getSettings()
      if (listenerScope.isCurrent(lifecycleRevision) && settingsRevision === initialRevision)
        applySettings(currentSettings)
    } catch (error) {
      if (!listenerScope.isCurrent(lifecycleRevision)) return
      barWidthDetails.value = `设置监听失败：${error instanceof Error ? error.message : String(error)}`
    }
  }

  /** 启动 Bar 页面唯一的一组全局事件监听，防止子组件重复订阅。 */
  async function start(): Promise<void> {
    if (listenerScope.isActive) return
    const lifecycleRevision = listenerScope.activate()
    // 媒体选择必须先于首次快照；设置监听与它没有依赖，可以同时初始化。
    await Promise.all([
      startSelectionListener(lifecycleRevision),
      startSettingsListener(lifecycleRevision),
    ])
    await startSnapshotListener(lifecycleRevision)
    if (listenerScope.isCurrent(lifecycleRevision)) markStartupMilestone('listeners-ready')
  }

  /** 销毁 Bar 页面建立的全部监听器。 */
  function stop(): void {
    listenerScope.deactivate()
    selectionRequestRevision += 1
    isWaitingForChangedTrackTimeline = false
    timelineResetAtUnixMs = null
    lastReportedMediaAvailable = undefined
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
