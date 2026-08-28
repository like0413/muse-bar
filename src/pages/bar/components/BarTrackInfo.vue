<script setup lang="ts">
import { usePreferredReducedMotion } from '@vueuse/core'
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

import {
  readElementAlignment,
  readTitleScrollEnabled,
  readTitleScrollMode,
  readTitleScrollSpeed,
} from '@/lib/settings-api'
import { cn } from '@/lib/utils'

import { useBarStore } from '../bar-store'

const barStore = useBarStore()
const preferredReducedMotion = usePreferredReducedMotion()
const { snapshot, settings, mediaStatus } = storeToRefs(barStore)

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
const elementAlignment = computed(() => readElementAlignment(settings.value))
const titleTrackClass = computed(() =>
  cn(
    'text-sm font-medium',
    isTitleScrolling.value ? 'inline-flex max-w-none whitespace-nowrap' : 'block w-full truncate',
    elementAlignment.value === 'right' && (isTitleScrolling.value ? 'ml-auto' : 'text-right'),
  ),
)
const artistClass = computed(() =>
  cn('text-muted-foreground truncate text-xs font-normal', {
    'text-right': elementAlignment.value === 'right',
  }),
)
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
  if (titleScrollMode.value === 'bounce') {
    const totalDuration = movementDuration * 2 + RESTART_TITLE_PAUSE_MS * 2
    const forwardStart = RESTART_TITLE_PAUSE_MS / totalDuration
    const forwardEnd = (RESTART_TITLE_PAUSE_MS + movementDuration) / totalDuration
    const backwardStart = (RESTART_TITLE_PAUSE_MS * 2 + movementDuration) / totalDuration
    titleAnimation = track.animate(
      [
        { transform: 'translateX(0)', offset: 0 },
        { transform: 'translateX(0)', offset: forwardStart },
        { transform: `translateX(-${distance}px)`, offset: forwardEnd },
        { transform: `translateX(-${distance}px)`, offset: backwardStart },
        { transform: 'translateX(0)', offset: 1 },
      ],
      {
        duration: totalDuration,
        easing: 'linear',
        iterations: Number.POSITIVE_INFINITY,
      },
    )
    return
  }

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
    preferredReducedMotion.value !== 'reduce' &&
    titleScrollEnabled.value &&
    viewportWidth > 0 &&
    textWidth > viewportWidth + 0.5
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

watch(
  [displayTitle, titleScrollEnabled, titleScrollSpeed, titleScrollMode, preferredReducedMotion],
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
  <div class="relative z-10 flex min-w-0 flex-1 cursor-default flex-col justify-center">
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
    <p v-if="displayArtist" data-bar-artist :class="artistClass">
      {{ displayArtist }}
    </p>
  </div>
</template>
