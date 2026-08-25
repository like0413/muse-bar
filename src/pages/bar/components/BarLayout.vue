<script setup lang="ts">
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

import { reportBarContentWidth } from '@/lib/bar-layout-api'
import { readControlPosition, readLyricsEnabled, readShowControls } from '@/lib/settings-api'

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
const { settings, snapshot } = storeToRefs(barStore)
const isHovered = shallowRef(false)
const lyricsEnabled = computed(() => readLyricsEnabled(settings.value))
const activeContent = computed(() => (lyricsEnabled.value && !isHovered.value ? 'lyrics' : 'media'))
const showMediaControls = computed(
  () => activeContent.value === 'media' && readShowControls(settings.value),
)
const controlPosition = computed(() => readControlPosition(settings.value))
const barPageElement = useTemplateRef<HTMLElement>('barPage')
const barSurfaceElement = useTemplateRef<HTMLElement>('barSurface')
const contentInitial = { opacity: 0, y: 2 }
const contentVisible = { opacity: 1, y: 0 }
const contentExit = { opacity: 0, y: -2 }
const contentTransition = { duration: 0.18, ease: [0.22, 1, 0.36, 1] as const }
let resizeObserver: ResizeObserver | undefined
let measurementFrame: number | undefined
let measurementRetryTimer: number | undefined
let lastReportedNaturalWidth = 0
let hasUnmounted = false

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
    void reportBarContentWidth(naturalWidth)
      .then((measurement) => {
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
        barStore.setBarWidthDetails(
          `宽度测量上报失败：${error instanceof Error ? error.message : String(error)}`,
        )
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

watch([() => snapshot.value?.title, () => snapshot.value?.artist], async () => {
  await nextTick()
  scheduleMeasurement()
})

watch(settings, async () => {
  lastReportedNaturalWidth = 0
  await nextTick()
  scheduleMeasurement()
})

onMounted(startMeasurement)
onBeforeUnmount(() => {
  hasUnmounted = true
  resizeObserver?.disconnect()
  if (measurementFrame !== undefined) window.cancelAnimationFrame(measurementFrame)
  if (measurementRetryTimer !== undefined) window.clearTimeout(measurementRetryTimer)
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
      @mouseleave="isHovered = false"
    >
      <BarProgress />
      <div data-bar-content class="absolute inset-y-0 right-2 left-2 z-10 flex items-center gap-2">
        <BarMediaControls v-if="showMediaControls && controlPosition === 'left'" />
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
        <BarMediaControls v-if="showMediaControls && controlPosition === 'right'" />
      </div>
    </section>
  </main>
</template>
