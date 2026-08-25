<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { computed, nextTick, onBeforeUnmount, onMounted, useTemplateRef, watch } from 'vue'

import { reportBarContentWidth } from '@/lib/bar-layout-api'
import { readControlPosition, readShowControls } from '@/lib/settings-api'

import { useBarStore } from '../bar-store'
import BarArtwork from './BarArtwork.vue'
import BarMediaControls from './BarMediaControls.vue'
import BarProgress from './BarProgress.vue'
import BarTrackInfo from './BarTrackInfo.vue'

const emit = defineEmits<{
  openSettings: []
}>()

const barStore = useBarStore()
const { settings, snapshot } = storeToRefs(barStore)
const showControls = computed(() => readShowControls(settings.value))
const controlPosition = computed(() => readControlPosition(settings.value))
const barPageElement = useTemplateRef<HTMLElement>('barPage')
const barSurfaceElement = useTemplateRef<HTMLElement>('barSurface')
let resizeObserver: ResizeObserver | undefined
let measurementFrame: number | undefined
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
  const title = surface?.querySelector<HTMLElement>('[data-bar-title]')
  const artwork = surface?.querySelector<HTMLElement>('[data-slot="avatar"]')
  const controls = surface?.querySelector<HTMLElement>('[data-slot="button-group"]')
  if (!page || !surface || !title || !artwork) return undefined

  const artist = surface.querySelector<HTMLElement>('[data-bar-artist]') ?? undefined
  const surfaceStyle = window.getComputedStyle(surface)
  const gap = readCssPixels(surfaceStyle.columnGap || surfaceStyle.gap)
  const textWidth = Math.max(measureTextContentWidth(title), measureTextContentWidth(artist))
  return (
    readHorizontalInsets(page) +
    readHorizontalInsets(surface) +
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
        barStore.setBarWidthDetails(
          `宽度：自然 ${measurement.naturalWidth.toFixed(1)}，目标 ${measurement.targetWidth}，范围 ${measurement.minimumWidth}–${measurement.maximumWidth}`,
        )
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
})
</script>

<template>
  <main ref="barPage" class="flex h-screen w-screen items-center justify-center bg-transparent">
    <section
      ref="barSurface"
      aria-label="Muse Bar"
      class="bg-secondary text-secondary-foreground relative flex h-full w-full items-center gap-2 overflow-hidden border px-2 text-sm font-medium"
      @contextmenu.prevent="emit('openSettings')"
    >
      <BarProgress />
      <BarMediaControls v-if="showControls && controlPosition === 'left'" />
      <BarArtwork />
      <BarTrackInfo />
      <BarMediaControls v-if="showControls && controlPosition === 'right'" />
    </section>
  </main>
</template>
