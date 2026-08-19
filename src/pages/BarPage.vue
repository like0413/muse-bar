<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

import {
  getCurrentMediaMetadata,
  getCurrentPlaybackCapabilities,
  getCurrentPlaybackStatus,
  getCurrentTimeline,
  listenToCurrentMediaMetadataChanges,
  listenToCurrentPlaybackCapabilitiesChanges,
  listenToCurrentPlaybackStatusChanges,
  listenToCurrentTimelineChanges,
  type CurrentMediaMetadata,
  type CurrentPlaybackCapabilities,
  type CurrentPlaybackStatus,
  type CurrentTimeline,
} from '@/lib/media-api'

const mediaMetadataStatus = ref('正在读取媒体信息')
const mediaMetadataDetails = ref('')
const playbackStatusText = ref('')
const playbackCapabilitiesText = ref('')
const timelineText = ref('')
const timelineDetails = ref('')
let stopMediaMetadataListener: UnlistenFn | undefined
let stopPlaybackCapabilitiesListener: UnlistenFn | undefined
let stopPlaybackStatusListener: UnlistenFn | undefined
let stopTimelineListener: UnlistenFn | undefined
let hasUnmounted = false

const mediaDetails = computed(() =>
  [
    mediaMetadataDetails.value,
    timelineDetails.value,
    playbackCapabilitiesText.value && `控制能力：${playbackCapabilitiesText.value}`,
  ]
    .filter(Boolean)
    .join('\n'),
)

const barSummary = computed(() =>
  [
    playbackStatusText.value,
    mediaMetadataStatus.value,
    timelineText.value,
    playbackCapabilitiesText.value,
  ]
    .filter(Boolean)
    .join(' · '),
)

const playbackStatusLabels: Record<CurrentPlaybackStatus, string> = {
  closed: '已关闭',
  opened: '已打开',
  changing: '切换中',
  stopped: '已停止',
  playing: '播放中',
  paused: '已暂停',
  unknown: '状态未知',
}

/** 将当前会话元数据转换为 Bar 的文本和完整悬停说明。 */
function showCurrentMediaMetadata(metadata: CurrentMediaMetadata | null) {
  if (!metadata) {
    mediaMetadataStatus.value = '当前没有媒体会话'
    mediaMetadataDetails.value = mediaMetadataStatus.value
    return
  }

  const title = metadata.title || '未知标题'
  mediaMetadataStatus.value = metadata.artist ? `${title} · ${metadata.artist}` : title
  mediaMetadataDetails.value = `${metadata.sourceAppId}\n标题：${title}\n歌手：${metadata.artist || '未知歌手'}`
}

/** 将 Windows 播放状态转换为当前验证页面使用的中文文本。 */
function showCurrentPlaybackStatus(status: CurrentPlaybackStatus | null) {
  playbackStatusText.value = status ? playbackStatusLabels[status] : '无播放状态'
}

/** 将播放器声明为可用的控制能力汇总为单行文本。 */
function showCurrentPlaybackCapabilities(capabilities: CurrentPlaybackCapabilities | null) {
  if (!capabilities) {
    playbackCapabilitiesText.value = '无控制能力'
    return
  }

  const enabledCapabilities = [
    capabilities.canPlay && '播放',
    capabilities.canPause && '暂停',
    capabilities.canPrevious && '上一曲',
    capabilities.canNext && '下一曲',
    capabilities.canSeek && 'Seek',
  ].filter(Boolean)

  playbackCapabilitiesText.value = enabledCapabilities.length
    ? enabledCapabilities.join('/')
    : '无可用控制'
}

/** 将毫秒时长转换为不受本地化影响的 mm:ss 文本。 */
function formatDuration(milliseconds: number) {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000))
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = String(totalSeconds % 60).padStart(2, '0')
  return `${minutes}:${seconds}`
}

/** 将有效时间轴转换为当前位置、总时长和诊断文本。 */
function showCurrentTimeline(timeline: CurrentTimeline | null) {
  if (!timeline) {
    timelineText.value = '无有效时间轴'
    timelineDetails.value = timelineText.value
    return
  }

  const elapsed = timeline.positionMs - timeline.startMs
  const duration = timeline.endMs - timeline.startMs
  const rate = timeline.playbackRate === null ? '速率未知' : `${timeline.playbackRate}x`
  timelineText.value = `${formatDuration(elapsed)}/${formatDuration(duration)} · ${rate}`
  timelineDetails.value = [
    `位置：${timeline.positionMs} ms`,
    `范围：${timeline.startMs}–${timeline.endMs} ms`,
    `Seek：${timeline.minSeekMs}–${timeline.maxSeekMs} ms`,
    `采样时间：${timeline.lastUpdatedAtUnixMs}`,
  ].join('\n')
}

/** 从 Rust 主动读取一次 Windows 当前会话元数据。 */
async function loadCurrentMediaMetadata() {
  try {
    showCurrentMediaMetadata(await getCurrentMediaMetadata())
  } catch {
    mediaMetadataStatus.value = '媒体信息读取失败'
    mediaMetadataDetails.value = mediaMetadataStatus.value
  }
}

/** 从 Rust 主动读取一次 Windows 当前会话播放状态。 */
async function loadCurrentPlaybackStatus() {
  try {
    showCurrentPlaybackStatus(await getCurrentPlaybackStatus())
  } catch {
    playbackStatusText.value = '状态读取失败'
  }
}

/** 从 Rust 主动读取一次 Windows 当前会话控制能力。 */
async function loadCurrentPlaybackCapabilities() {
  try {
    showCurrentPlaybackCapabilities(await getCurrentPlaybackCapabilities())
  } catch {
    playbackCapabilitiesText.value = '能力读取失败'
  }
}

/** 从 Rust 主动读取一次 Windows 当前会话时间轴。 */
async function loadCurrentTimeline() {
  try {
    showCurrentTimeline(await getCurrentTimeline())
  } catch {
    timelineText.value = '时间轴读取失败'
    timelineDetails.value = timelineText.value
  }
}

/** 先建立元数据事件订阅，再读取当前值，避免页面初始化期间遗漏切歌。 */
async function startMediaMetadataListener() {
  try {
    const stopListener = await listenToCurrentMediaMetadataChanges(showCurrentMediaMetadata)
    if (hasUnmounted) {
      stopListener()
      return
    }

    stopMediaMetadataListener = stopListener
    await loadCurrentMediaMetadata()
  } catch {
    mediaMetadataStatus.value = '媒体信息监听失败'
    mediaMetadataDetails.value = mediaMetadataStatus.value
  }
}

/** 先建立播放状态订阅，再读取当前状态，避免页面初始化期间遗漏变化。 */
async function startPlaybackStatusListener() {
  try {
    const stopListener = await listenToCurrentPlaybackStatusChanges(showCurrentPlaybackStatus)
    if (hasUnmounted) {
      stopListener()
      return
    }

    stopPlaybackStatusListener = stopListener
    await loadCurrentPlaybackStatus()
  } catch {
    playbackStatusText.value = '状态监听失败'
  }
}

/** 先建立控制能力订阅，再读取当前值，避免初始化期间遗漏变化。 */
async function startPlaybackCapabilitiesListener() {
  try {
    const stopListener = await listenToCurrentPlaybackCapabilitiesChanges(
      showCurrentPlaybackCapabilities,
    )
    if (hasUnmounted) {
      stopListener()
      return
    }

    stopPlaybackCapabilitiesListener = stopListener
    await loadCurrentPlaybackCapabilities()
  } catch {
    playbackCapabilitiesText.value = '能力监听失败'
  }
}

/** 先建立时间轴订阅，再读取当前快照，避免初始化期间遗漏变化。 */
async function startTimelineListener() {
  try {
    const stopListener = await listenToCurrentTimelineChanges(showCurrentTimeline)
    if (hasUnmounted) {
      stopListener()
      return
    }

    stopTimelineListener = stopListener
    await loadCurrentTimeline()
  } catch {
    timelineText.value = '时间轴监听失败'
    timelineDetails.value = timelineText.value
  }
}

onMounted(startMediaMetadataListener)
onMounted(startPlaybackCapabilitiesListener)
onMounted(startPlaybackStatusListener)
onMounted(startTimelineListener)

onBeforeUnmount(() => {
  hasUnmounted = true
  stopMediaMetadataListener?.()
  stopPlaybackCapabilitiesListener?.()
  stopPlaybackStatusListener?.()
  stopTimelineListener?.()
})
</script>

<template>
  <main class="flex h-screen w-screen items-center justify-center bg-transparent p-1">
    <section
      aria-label="Muse Bar"
      class="bg-secondary text-secondary-foreground flex h-full w-full items-center justify-center rounded-md border px-3 text-sm font-medium"
    >
      <span class="truncate" :title="mediaDetails">
        {{ barSummary }}
      </span>
    </section>
  </main>
</template>
