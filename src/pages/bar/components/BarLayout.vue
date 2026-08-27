<script setup lang="ts">
import { usePreferredReducedMotion } from '@vueuse/core'
import { AnimatePresence, motion } from 'motion-v'
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
  controlCurrentApplicationVolume,
  getCurrentApplicationVolume,
  hideApplicationVolumeFlyout,
  listenToApplicationVolumeStateChanged,
  listenToVolumeFlyoutHidden,
  listenToVolumeFlyoutHoverChanged,
  readVolumeWheelDelta,
  showApplicationVolumeFlyout,
  type ApplicationVolumeAction,
  type ApplicationVolumeState,
  type VolumeFlyoutAnchor,
} from '@/lib/application-volume-api'
import { reportBarContentWidth } from '@/lib/bar-layout-api'
import {
  readElementAlignment,
  readLyricsEnabled,
  readProgressColor,
  readShowControls,
} from '@/lib/settings-api'
import { TauriListenerScope } from '@/lib/tauri-listener-scope'
import { cn, getErrorMessage } from '@/lib/utils'

import { useBarStore } from '../bar-store'
import BarArtwork from './BarArtwork.vue'
import BarLyricsContent from './BarLyricsContent.vue'
import BarMediaControls from './BarMediaControls.vue'
import BarProgress from './BarProgress.vue'
import BarTrackInfo from './BarTrackInfo.vue'

const emit = defineEmits<{
  openSettings: []
}>()

const barStore = useBarStore()
const preferredReducedMotion = usePreferredReducedMotion()
const { settings, snapshot } = storeToRefs(barStore)
const isHovered = shallowRef(false)
const volumeState = shallowRef<ApplicationVolumeState | null>(null)
const volumeUnavailableReason = shallowRef('正在读取当前应用音量')
const volumePending = shallowRef(false)
const volumeAnchor = shallowRef<VolumeFlyoutAnchor>()
const volumeButtonHovered = shallowRef(false)
const volumeFlyoutHovered = shallowRef(false)
const volumeInteractionActive = shallowRef(false)
const lyricsEnabled = computed(() => readLyricsEnabled(settings.value))
const activeContent = computed(() =>
  lyricsEnabled.value && !isHovered.value && !volumeInteractionActive.value ? 'lyrics' : 'media',
)
const showMediaControls = computed(
  () => activeContent.value === 'media' && readShowControls(settings.value),
)
const elementAlignment = computed(() => readElementAlignment(settings.value))
const progressColor = computed(() =>
  readProgressColor(settings.value, snapshot.value?.accentColor, snapshot.value?.systemAccentColor),
)
const contentClass = computed(() =>
  cn('absolute inset-y-0 right-2 left-2 z-10 flex items-center gap-2', {
    'flex-row-reverse': elementAlignment.value === 'right',
  }),
)
const barPageElement = useTemplateRef<HTMLElement>('barPage')
const barSurfaceElement = useTemplateRef<HTMLElement>('barSurface')
const contentInitial = { opacity: 0, y: 2 }
const contentVisible = { opacity: 1, y: 0 }
const contentExit = { opacity: 0, y: -2 }
const contentTransition = computed(() =>
  preferredReducedMotion.value === 'reduce'
    ? { duration: 0 }
    : { duration: 0.18, ease: [0.22, 1, 0.36, 1] as const },
)
let resizeObserver: ResizeObserver | undefined
let measurementFrame: number | undefined
let measurementRetryTimer: number | undefined
let lastReportedNaturalWidth = 0
let hasUnmounted = false
let measurementRevision = 0
const volumeListenerScope = new TauriListenerScope()
let volumeRequestRevision = 0
let hideFlyoutTimer: number | undefined
let wheelFeedbackTimer: number | undefined
let wheelAccumulator = 0
let queuedAdjustment = 0
let isApplyingAdjustment = false

/** 将 getComputedStyle 返回的像素文本安全转换为数值。 */
function readCssPixels(value: string): number {
  const pixels = Number.parseFloat(value)
  return Number.isFinite(pixels) ? pixels : 0
}

/** 计算元素左右方向的内边距与边框总宽度。 */
function readHorizontalInsets(element: HTMLElement): number {
  const style = window.getComputedStyle(element)
  return (
    readCssPixels(style.paddingLeft) +
    readCssPixels(style.paddingRight) +
    readCssPixels(style.borderLeftWidth) +
    readCssPixels(style.borderRightWidth)
  )
}

/** 使用 DOM Range 读取文本本身的渲染宽度，不受当前 flex 容器宽度影响。 */
function measureTextContentWidth(element: HTMLElement | undefined): number {
  if (!element?.textContent) return 0
  const range = document.createRange()
  range.selectNodeContents(element)
  return range.getBoundingClientRect().width
}

/** 汇总封面、自然文本、控制按钮、间距和容器边界所需的完整逻辑宽度。 */
function measureNaturalWidth(): number | undefined {
  const page = barPageElement.value
  const surface = barSurfaceElement.value
  if (!page || !surface) return undefined
  // 歌词模式始终保持任务栏可用区域宽度，固定哨兵值避免悬停切换触发原生缩放。
  if (lyricsEnabled.value) return 1

  const content = surface.querySelector<HTMLElement>('[data-bar-content]')
  const title = content?.querySelector<HTMLElement>('[data-bar-title]')
  const artwork = content?.querySelector<HTMLElement>('[data-slot="avatar"]')
  const controls = content?.querySelector<HTMLElement>('[data-slot="button-group"]')
  if (!content || !title || !artwork) return undefined

  const artist = content.querySelector<HTMLElement>('[data-bar-artist]') ?? undefined
  const contentStyle = window.getComputedStyle(content)
  const gap = readCssPixels(contentStyle.columnGap || contentStyle.gap)
  const textWidth = Math.max(measureTextContentWidth(title), measureTextContentWidth(artist))
  return (
    readHorizontalInsets(page) +
    readHorizontalInsets(surface) +
    readHorizontalInsets(content) +
    artwork.getBoundingClientRect().width +
    (controls?.getBoundingClientRect().width ?? 0) +
    textWidth +
    gap * (controls ? 2 : 1)
  )
}

/** 在下一帧合并连续布局变化，并将新的自然宽度上报给 Rust。 */
function scheduleMeasurement(): void {
  if (hasUnmounted || measurementFrame !== undefined) return

  measurementFrame = window.requestAnimationFrame(() => {
    measurementFrame = undefined
    const naturalWidth = measureNaturalWidth()
    if (naturalWidth === undefined || Math.abs(naturalWidth - lastReportedNaturalWidth) < 0.5) {
      return
    }

    lastReportedNaturalWidth = naturalWidth
    const revision = ++measurementRevision
    void reportBarContentWidth(naturalWidth, preferredReducedMotion.value === 'reduce')
      .then((measurement) => {
        if (hasUnmounted || revision !== measurementRevision) return
        const details =
          measurement.mode === 'availableArea'
            ? `宽度：歌词可用区域 ${measurement.targetWidth}`
            : `宽度：自然 ${measurement.naturalWidth.toFixed(1)}，目标 ${measurement.targetWidth}，上限 ${measurement.maximumWidth}`
        barStore.setBarWidthDetails(details)
        if (!measurement.applied) {
          // 切换目标显示器时原生 Child 需要先重新挂载，稍后再应用同一次宽度策略。
          lastReportedNaturalWidth = 0
          if (measurementRetryTimer !== undefined) window.clearTimeout(measurementRetryTimer)
          measurementRetryTimer = window.setTimeout(() => {
            measurementRetryTimer = undefined
            scheduleMeasurement()
          }, 400)
        }
      })
      .catch((error: unknown) => {
        if (hasUnmounted || revision !== measurementRevision) return
        barStore.setBarWidthDetails(`宽度测量上报失败：${getErrorMessage(error)}`)
      })
  })
}

/** 观察 Bar 容器尺寸，并在字体加载完成后补做一次测量。 */
function startMeasurement(): void {
  const surface = barSurfaceElement.value
  if (!surface) return

  resizeObserver = new ResizeObserver(scheduleMeasurement)
  resizeObserver.observe(surface)
  scheduleMeasurement()
  void document.fonts.ready.then(() => {
    if (!hasUnmounted) scheduleMeasurement()
  })
}

async function refreshVolume(): Promise<void> {
  const sessionKey = snapshot.value?.sessionKey
  const requestRevision = ++volumeRequestRevision
  if (!showMediaControls.value || sessionKey === undefined) {
    volumeState.value = null
    volumeUnavailableReason.value = '当前没有媒体会话'
    return
  }
  try {
    const state = await getCurrentApplicationVolume(sessionKey)
    if (requestRevision !== volumeRequestRevision || snapshot.value?.sessionKey !== sessionKey)
      return
    volumeState.value = state
    volumeUnavailableReason.value = state ? '' : '当前应用没有可用的音频会话'
  } catch (error) {
    if (requestRevision !== volumeRequestRevision) return
    volumeState.value = null
    volumeUnavailableReason.value = `无法读取当前应用音量：${getErrorMessage(error)}`
  }
}

async function controlVolume(action: ApplicationVolumeAction): Promise<void> {
  const sessionKey = snapshot.value?.sessionKey
  if (sessionKey === undefined || volumePending.value) return
  volumePending.value = true
  try {
    volumeState.value = await controlCurrentApplicationVolume(sessionKey, action)
    volumeUnavailableReason.value = ''
  } catch (error) {
    volumeUnavailableReason.value = `音量控制失败：${getErrorMessage(error)}`
    await refreshVolume()
  } finally {
    volumePending.value = false
  }
}

async function drainVolumeAdjustments(): Promise<void> {
  if (isApplyingAdjustment) return
  isApplyingAdjustment = true
  try {
    while (queuedAdjustment !== 0) {
      const deltaPercent = queuedAdjustment
      queuedAdjustment = 0
      await controlVolume({ type: 'adjust', deltaPercent })
    }
  } finally {
    isApplyingAdjustment = false
  }
}

function enqueueVolumeAdjustment(deltaPercent: number): void {
  queuedAdjustment = Math.max(-20, Math.min(20, queuedAdjustment + deltaPercent))
  void drainVolumeAdjustments()
}

function clearHideFlyoutTimer(): void {
  if (hideFlyoutTimer !== undefined) window.clearTimeout(hideFlyoutTimer)
  hideFlyoutTimer = undefined
}

function scheduleFlyoutHide(): void {
  clearHideFlyoutTimer()
  if (volumeButtonHovered.value || volumeFlyoutHovered.value || wheelFeedbackTimer !== undefined)
    return
  hideFlyoutTimer = window.setTimeout(() => {
    hideFlyoutTimer = undefined
    volumeInteractionActive.value = false
    void hideApplicationVolumeFlyout()
  }, 250)
}

function showVolumeFlyout(): void {
  const anchor = volumeAnchor.value
  const sessionKey = snapshot.value?.sessionKey
  if (!anchor || sessionKey === undefined || !volumeState.value || !showMediaControls.value) return
  clearHideFlyoutTimer()
  volumeInteractionActive.value = true
  void showApplicationVolumeFlyout(anchor, sessionKey, progressColor.value).catch(
    (error: unknown) => {
      volumeUnavailableReason.value = `音量浮层打开失败：${getErrorMessage(error)}`
    },
  )
}

async function handleVolumeAnchorEntered(): Promise<void> {
  volumeButtonHovered.value = true
  if (!volumeState.value) await refreshVolume()
  if (!volumeButtonHovered.value) return
  showVolumeFlyout()
}

function handleVolumeAnchorLeft(): void {
  volumeButtonHovered.value = false
  scheduleFlyoutHide()
}

function showWheelFeedback(): void {
  showVolumeFlyout()
  if (wheelFeedbackTimer !== undefined) window.clearTimeout(wheelFeedbackTimer)
  wheelFeedbackTimer = window.setTimeout(() => {
    wheelFeedbackTimer = undefined
    scheduleFlyoutHide()
  }, 1200)
}

async function applyWheelAdjustment(deltaPercent: number): Promise<void> {
  if (!volumeState.value) await refreshVolume()
  if (!volumeState.value) return
  enqueueVolumeAdjustment(deltaPercent)
  showWheelFeedback()
}

function handleVolumeWheel(event: WheelEvent): void {
  if (!showMediaControls.value || !volumeAnchor.value) return
  const delta = readVolumeWheelDelta(event)
  if (delta === null) return
  event.preventDefault()
  if (wheelAccumulator !== 0 && Math.sign(wheelAccumulator) !== Math.sign(delta)) {
    wheelAccumulator = 0
  }
  wheelAccumulator += delta
  if (Math.abs(wheelAccumulator) < 40) return
  const deltaPercent = wheelAccumulator < 0 ? 2 : -2
  wheelAccumulator = 0
  void applyWheelAdjustment(deltaPercent)
}

async function toggleVolumeMute(): Promise<void> {
  if (!volumeState.value) await refreshVolume()
  if (volumeState.value) await controlVolume({ type: 'toggleMute' })
}

function handleBarMouseLeave(): void {
  isHovered.value = false
  scheduleFlyoutHide()
}

watch([() => snapshot.value?.title, () => snapshot.value?.artist], async () => {
  await nextTick()
  scheduleMeasurement()
})

watch(settings, async () => {
  lastReportedNaturalWidth = 0
  await nextTick()
  scheduleMeasurement()
})

watch(
  [() => snapshot.value?.sessionKey, showMediaControls],
  ([, controlsVisible]) => {
    queuedAdjustment = 0
    wheelAccumulator = 0
    if (!controlsVisible) {
      volumeInteractionActive.value = false
      volumeRequestRevision += 1
      volumeState.value = null
      void hideApplicationVolumeFlyout()
      return
    }
    void refreshVolume()
  },
  { immediate: true },
)

onMounted(() => {
  startMeasurement()
  const lifecycleRevision = volumeListenerScope.activate()
  void volumeListenerScope.register(
    lifecycleRevision,
    listenToVolumeFlyoutHoverChanged((hovered) => {
      volumeFlyoutHovered.value = hovered
      if (hovered) clearHideFlyoutTimer()
      else scheduleFlyoutHide()
    }),
  )
  void volumeListenerScope.register(
    lifecycleRevision,
    listenToApplicationVolumeStateChanged((nextState) => {
      if (snapshot.value?.sessionKey === nextState.sessionKey) volumeState.value = nextState
    }),
  )
  void volumeListenerScope.register(
    lifecycleRevision,
    listenToVolumeFlyoutHidden(() => {
      volumeFlyoutHovered.value = false
      volumeButtonHovered.value = false
      volumeInteractionActive.value = false
    }),
  )
})
onBeforeUnmount(() => {
  hasUnmounted = true
  measurementRevision += 1
  resizeObserver?.disconnect()
  if (measurementFrame !== undefined) window.cancelAnimationFrame(measurementFrame)
  if (measurementRetryTimer !== undefined) window.clearTimeout(measurementRetryTimer)
  volumeListenerScope.deactivate()
  clearHideFlyoutTimer()
  if (wheelFeedbackTimer !== undefined) window.clearTimeout(wheelFeedbackTimer)
  void hideApplicationVolumeFlyout()
})
</script>

<template>
  <main ref="barPage" class="flex h-screen w-screen items-center justify-center bg-transparent">
    <section
      ref="barSurface"
      aria-label="Muse Bar"
      class="bg-secondary text-secondary-foreground relative flex h-full w-full items-center gap-2 overflow-hidden border px-2 text-sm font-medium"
      @contextmenu.prevent="emit('openSettings')"
      @mouseenter="isHovered = true"
      @mouseleave="handleBarMouseLeave"
    >
      <BarProgress />
      <div data-bar-content :class="contentClass">
        <BarArtwork />
        <div class="relative h-full min-w-0 flex-1">
          <AnimatePresence :initial="false" mode="sync">
            <motion.div
              :key="activeContent"
              class="absolute inset-0 flex items-center"
              :initial="contentInitial"
              :animate="contentVisible"
              :exit="contentExit"
              :transition="contentTransition"
            >
              <BarLyricsContent v-if="activeContent === 'lyrics'" />
              <BarTrackInfo v-else />
            </motion.div>
          </AnimatePresence>
        </div>
        <BarMediaControls
          v-if="showMediaControls"
          :volume-state="volumeState"
          :volume-unavailable-reason="volumeUnavailableReason"
          :volume-pending="volumePending"
          @volume-anchor-changed="volumeAnchor = $event"
          @volume-anchor-entered="handleVolumeAnchorEntered"
          @volume-anchor-left="handleVolumeAnchorLeft"
          @volume-wheel="handleVolumeWheel"
          @toggle-volume-mute="toggleVolumeMute"
        />
      </div>
    </section>
  </main>
</template>
