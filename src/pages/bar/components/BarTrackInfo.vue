<script setup lang="ts">
import { storeToRefs } from 'pinia'
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  shallowRef,
  useTemplateRef,
  watch,
} from 'vue'

import type { CurrentPlaybackStatus, CurrentTimeline } from '@/lib/media-api'
import {
  readTitleScrollEnabled,
  readTitleScrollMode,
  readTitleScrollSpeed,
} from '@/lib/settings-api'
import { cn } from '@/lib/utils'

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
  settings,
  mediaStatus,
  mediaSelectionText,
  barWidthDetails,
  settingsWindowError,
  controlError,
} = storeToRefs(barStore)

const CONTINUOUS_TITLE_GAP = 24
const RESTART_TITLE_PAUSE_MS = 800
const titleViewportElement = useTemplateRef<HTMLElement>('titleViewport')
const titleTrackElement = useTemplateRef<HTMLElement>('titleTrack')
const primaryTitleElement = useTemplateRef<HTMLElement>('primaryTitle')
const isTitleScrolling = shallowRef(false)
let titleAnimation: Animation | undefined
let titleResizeObserver: ResizeObserver | undefined
let titleRefreshTimer: number | undefined
let titleRefreshRevision = 0

const displayTitle = computed(() => snapshot.value?.title || mediaStatus.value)
const displayArtist = computed(() => snapshot.value?.artist ?? '')
const titleScrollEnabled = computed(() => readTitleScrollEnabled(settings.value))
const titleScrollSpeed = computed(() => readTitleScrollSpeed(settings.value))
const titleScrollMode = computed(() => readTitleScrollMode(settings.value))
const titleTrackClass = computed(() =>
  cn(
    'text-sm font-medium',
    isTitleScrolling.value ? 'inline-flex max-w-none whitespace-nowrap' : 'block w-full truncate',
  ),
)
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

/** 使用 DOM Range 测量文字自然宽度，不受当前标题容器的截断宽度影响。 */
function measureTitleWidth(element: HTMLElement): number {
  const range = document.createRange()
  range.selectNodeContents(element)
  return range.getBoundingClientRect().width
}

/** 停止旧标题动画，防止切歌、改速或窗口缩放后多个动画同时修改 transform。 */
function stopTitleAnimation(): void {
  titleAnimation?.cancel()
  titleAnimation = undefined
}

/** 根据溢出距离和设置速度创建当前滚动方式对应的匀速动画。 */
function startTitleAnimation(textWidth: number, viewportWidth: number): void {
  const track = titleTrackElement.value
  if (!track) return

  const speed = Math.max(1, titleScrollSpeed.value)
  if (titleScrollMode.value === 'continuous') {
    const distance = textWidth + CONTINUOUS_TITLE_GAP
    titleAnimation = track.animate(
      [{ transform: 'translateX(0)' }, { transform: `translateX(-${distance}px)` }],
      {
        duration: (distance / speed) * 1000,
        easing: 'linear',
        iterations: Number.POSITIVE_INFINITY,
      },
    )
    return
  }

  const distance = textWidth - viewportWidth
  const movementDuration = (distance / speed) * 1000
  const totalDuration = movementDuration + RESTART_TITLE_PAUSE_MS * 2
  const movementStart = RESTART_TITLE_PAUSE_MS / totalDuration
  const movementEnd = (RESTART_TITLE_PAUSE_MS + movementDuration) / totalDuration
  titleAnimation = track.animate(
    [
      { transform: 'translateX(0)', offset: 0 },
      { transform: 'translateX(0)', offset: movementStart },
      { transform: `translateX(-${distance}px)`, offset: movementEnd },
      { transform: `translateX(-${distance}px)`, offset: 1 },
    ],
    {
      duration: totalDuration,
      easing: 'linear',
      iterations: Number.POSITIVE_INFINITY,
    },
  )
}

/** 重新判断标题是否溢出，并用最新设置重建滚动动画。 */
async function refreshTitleScrolling(): Promise<void> {
  const revision = ++titleRefreshRevision
  stopTitleAnimation()
  await nextTick()
  if (revision !== titleRefreshRevision) return

  const viewport = titleViewportElement.value
  const primaryTitle = primaryTitleElement.value
  if (!viewport || !primaryTitle) return

  const textWidth = measureTitleWidth(primaryTitle)
  const viewportWidth = viewport.clientWidth
  const shouldScroll =
    titleScrollEnabled.value && viewportWidth > 0 && textWidth > viewportWidth + 0.5
  isTitleScrolling.value = shouldScroll
  if (!shouldScroll) return

  // 连续模式需要等待第二份标题进入 DOM，之后才能在完整轨道上启动动画。
  await nextTick()
  if (revision !== titleRefreshRevision) return
  startTitleAnimation(textWidth, viewportWidth)
}

/** 合并同一轮布局中的连续变化，避免宽度动画期间同步重复测量。 */
function scheduleTitleScrollingRefresh(): void {
  if (titleRefreshTimer !== undefined) window.clearTimeout(titleRefreshTimer)
  titleRefreshTimer = window.setTimeout(() => {
    titleRefreshTimer = undefined
    void refreshTitleScrolling()
  }, 0)
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

watch(
  [displayTitle, titleScrollEnabled, titleScrollSpeed, titleScrollMode],
  scheduleTitleScrollingRefresh,
  { flush: 'post' },
)

onMounted(() => {
  const viewport = titleViewportElement.value
  if (viewport) {
    titleResizeObserver = new ResizeObserver(scheduleTitleScrollingRefresh)
    titleResizeObserver.observe(viewport)
  }
  scheduleTitleScrollingRefresh()
})

onBeforeUnmount(() => {
  titleRefreshRevision += 1
  titleResizeObserver?.disconnect()
  stopTitleAnimation()
  if (titleRefreshTimer !== undefined) window.clearTimeout(titleRefreshTimer)
})
</script>

<template>
  <div class="relative z-10 flex min-w-0 flex-1 flex-col justify-center" :title="mediaDetails">
    <div ref="titleViewport" class="w-full overflow-hidden">
      <p ref="titleTrack" :class="titleTrackClass">
        <span ref="primaryTitle" data-bar-title class="shrink-0">{{ displayTitle }}</span>
        <span
          v-if="isTitleScrolling && titleScrollMode === 'continuous'"
          aria-hidden="true"
          class="ml-6 shrink-0"
          >{{ displayTitle }}</span
        >
      </p>
    </div>
    <p
      v-if="displayArtist"
      data-bar-artist
      class="text-muted-foreground truncate text-xs font-normal"
    >
      {{ displayArtist }}
    </p>
  </div>
</template>
