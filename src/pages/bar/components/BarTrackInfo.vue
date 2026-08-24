<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { computed } from 'vue'

import type { CurrentPlaybackStatus, CurrentTimeline } from '@/lib/media-api'

import { useBarStore } from '../bar-store'

const playbackStatusLabels: Record<CurrentPlaybackStatus, string> = {
  closed: '已关闭',
  opened: '已打开',
  changing: '切换中',
  stopped: '已停止',
  playing: '播放中',
  paused: '已暂停',
  unknown: '状态未知',
}

const barStore = useBarStore()
const {
  snapshot,
  mediaStatus,
  mediaSelectionText,
  barWidthDetails,
  settingsWindowError,
  controlError,
} = storeToRefs(barStore)

const displayTitle = computed(() => snapshot.value?.title || mediaStatus.value)
const displayArtist = computed(() => snapshot.value?.artist ?? '')
const metadataDetails = computed(() => {
  const media = snapshot.value
  if (!media) return mediaStatus.value
  return `${media.sourceAppId}\n标题：${media.title || '未知标题'}\n歌手：${media.artist || '未知歌手'}`
})
const playbackStatusText = computed(() => {
  const status = snapshot.value?.playbackStatus
  return status ? playbackStatusLabels[status] : '无播放状态'
})
const playbackCapabilitiesText = computed(() => {
  const capabilities = snapshot.value?.capabilities
  if (!capabilities) return '无控制能力'

  const enabledCapabilities = [
    capabilities.canPlay && '播放',
    capabilities.canPause && '暂停',
    capabilities.canPrevious && '上一曲',
    capabilities.canNext && '下一曲',
    capabilities.canSeek && 'Seek',
  ].filter(Boolean)
  return enabledCapabilities.length ? enabledCapabilities.join('/') : '无可用控制'
})

/** 将毫秒时长转换为不受本地化影响的 mm:ss 文本。 */
function formatDuration(milliseconds: number): string {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000))
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = String(totalSeconds % 60).padStart(2, '0')
  return `${minutes}:${seconds}`
}

/** 将时间轴转换为用于悬停诊断的多行文本。 */
function formatTimelineDetails(timeline: CurrentTimeline | null | undefined): string {
  if (!timeline) return '无有效时间轴'

  const elapsed = timeline.positionMs - timeline.startMs
  const duration = timeline.endMs - timeline.startMs
  const rate = timeline.playbackRate === null ? '速率未知' : `${timeline.playbackRate}x`
  return [
    `进度：${formatDuration(elapsed)}/${formatDuration(duration)} · ${rate}`,
    `位置：${timeline.positionMs} ms`,
    `范围：${timeline.startMs}–${timeline.endMs} ms`,
    `Seek：${timeline.minSeekMs}–${timeline.maxSeekMs} ms`,
    `采样时间：${timeline.lastUpdatedAtUnixMs}`,
  ].join('\n')
}

const mediaDetails = computed(() =>
  [
    metadataDetails.value,
    `播放状态：${playbackStatusText.value}`,
    formatTimelineDetails(snapshot.value?.timeline),
    `控制能力：${playbackCapabilitiesText.value}`,
    mediaSelectionText.value,
    barWidthDetails.value,
    settingsWindowError.value,
    controlError.value,
  ]
    .filter(Boolean)
    .join('\n'),
)
</script>

<template>
  <div class="relative z-10 flex min-w-0 flex-1 flex-col justify-center" :title="mediaDetails">
    <p data-bar-title class="truncate text-sm font-medium">{{ displayTitle }}</p>
    <p
      v-if="displayArtist"
      data-bar-artist
      class="text-muted-foreground truncate text-xs font-normal"
    >
      {{ displayArtist }}
    </p>
  </div>
</template>
