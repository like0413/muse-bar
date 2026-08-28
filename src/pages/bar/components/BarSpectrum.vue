<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from 'vue'

import type { CurrentPlaybackStatus } from '@/lib/media-types'
import type { SpectrumOrigin } from '@/lib/settings-api'
import {
  listenToBarVisibilityChanges,
  listenToSpectrumFrames,
  startApplicationSpectrum,
  stopApplicationSpectrum,
} from '@/lib/spectrum-api'
import { TauriListenerScope } from '@/lib/tauri-listener-scope'

const props = defineProps<{
  sessionKey: number
  playbackStatus: CurrentPlaybackStatus
  accentColor: string
  frameRate: 20 | 30
  origin: SpectrumOrigin
}>()

const BAND_COUNT = 16
const CAPTURE_RETRY_MS = 1500
const LEVEL_SETTLE_EPSILON = 0.001
const canvas = useTemplateRef<HTMLCanvasElement>('canvas')
const isBarVisible = shallowRef(true)
const shouldCapture = computed(() => isBarVisible.value && props.playbackStatus === 'playing')
const resolvedAccentColor = computed(() =>
  /^#[0-9a-f]{6}$/i.test(props.accentColor) ? props.accentColor : '#0078D4',
)
const listenerScope = new TauriListenerScope()
const currentLevels = new Float32Array(BAND_COUNT)
const targetLevels = new Float32Array(BAND_COUNT)
let animationFrame: number | undefined
let captureRetryTimer: number | undefined
let resizeObserver: ResizeObserver | undefined
let captureRevision = 0
let lastDrawAt = 0

function resizeCanvas(): void {
  const element = canvas.value
  if (!element) return
  const bounds = element.getBoundingClientRect()
  const scale = Math.min(window.devicePixelRatio || 1, 2)
  const width = Math.max(1, Math.round(bounds.width * scale))
  const height = Math.max(1, Math.round(bounds.height * scale))
  if (element.width !== width) element.width = width
  if (element.height !== height) element.height = height
}

function drawFrame(timestamp: number): void {
  animationFrame = undefined
  if (timestamp - lastDrawAt < 1000 / props.frameRate) {
    animationFrame = window.requestAnimationFrame(drawFrame)
    return
  }
  lastDrawAt = timestamp
  const element = canvas.value
  const context = element?.getContext('2d')
  if (!element || !context) return

  context.clearRect(0, 0, element.width, element.height)
  const gap = Math.max(2, element.width * 0.006)
  const barWidth = Math.max(1, (element.width - gap * (BAND_COUNT - 1)) / BAND_COUNT)
  const maximumHeight = element.height
  let stillAnimating = false

  context.fillStyle = resolvedAccentColor.value
  context.globalAlpha = 0.9
  context.beginPath()
  for (let index = 0; index < BAND_COUNT; index += 1) {
    const target = shouldCapture.value ? targetLevels[index] : 0
    const easing = target > currentLevels[index] ? 0.48 : 0.2
    currentLevels[index] += (target - currentLevels[index]) * easing
    if (Math.abs(target - currentLevels[index]) <= LEVEL_SETTLE_EPSILON) {
      currentLevels[index] = target
    } else {
      stillAnimating = true
    }
    const height = Math.max(1, maximumHeight * currentLevels[index])
    const x = index * (barWidth + gap)
    const y = props.origin === 'center' ? (element.height - height) / 2 : element.height - height
    const radius = Math.min(barWidth / 2, height / 2)
    context.roundRect(x, y, barWidth, height, radius)
  }
  context.fill()

  if (stillAnimating) animationFrame = window.requestAnimationFrame(drawFrame)
}

function ensureAnimation(): void {
  if (animationFrame === undefined) animationFrame = window.requestAnimationFrame(drawFrame)
}

function applyFrame(levels: number[]): void {
  for (let index = 0; index < BAND_COUNT; index += 1) {
    const level = levels[index]
    targetLevels[index] =
      typeof level === 'number' && Number.isFinite(level) ? Math.min(1, Math.max(0, level)) : 0
  }
  ensureAnimation()
}

function clearCaptureRetry(): void {
  if (captureRetryTimer !== undefined) window.clearTimeout(captureRetryTimer)
  captureRetryTimer = undefined
}

async function synchronizeCapture(): Promise<void> {
  const revision = ++captureRevision
  clearCaptureRetry()
  if (!shouldCapture.value) {
    targetLevels.fill(0)
    ensureAnimation()
    try {
      await stopApplicationSpectrum(props.sessionKey)
    } catch {
      // 生命周期兜底也会停止采集，前端无需用错误状态干扰 Bar。
    }
    return
  }

  try {
    await startApplicationSpectrum(props.sessionKey, props.frameRate)
  } catch {
    if (revision !== captureRevision || !shouldCapture.value) return
    targetLevels.fill(0)
    ensureAnimation()
    captureRetryTimer = window.setTimeout(() => {
      captureRetryTimer = undefined
      void synchronizeCapture()
    }, CAPTURE_RETRY_MS)
  }
}

watch(
  [() => props.sessionKey, () => props.frameRate, shouldCapture],
  () => void synchronizeCapture(),
)
watch(
  () => props.accentColor,
  () => ensureAnimation(),
)

onMounted(() => {
  const element = canvas.value
  if (element) {
    resizeCanvas()
    resizeObserver = new ResizeObserver(() => {
      resizeCanvas()
      ensureAnimation()
    })
    resizeObserver.observe(element.parentElement ?? element)
  }
  const lifecycleRevision = listenerScope.activate()
  void listenerScope.register(
    lifecycleRevision,
    listenToSpectrumFrames((frame) => {
      if (frame.sessionKey === props.sessionKey) applyFrame(frame.levels)
    }),
  )
  void listenerScope.register(
    lifecycleRevision,
    listenToBarVisibilityChanges((visible) => {
      if (visible === isBarVisible.value) return
      isBarVisible.value = visible
    }),
  )
  void synchronizeCapture()
})

onBeforeUnmount(() => {
  captureRevision += 1
  listenerScope.deactivate()
  resizeObserver?.disconnect()
  clearCaptureRetry()
  if (animationFrame !== undefined) window.cancelAnimationFrame(animationFrame)
  void stopApplicationSpectrum(props.sessionKey).catch(() => undefined)
})
</script>

<template>
  <canvas
    ref="canvas"
    data-bar-spectrum
    aria-hidden="true"
    class="pointer-events-none h-full w-16 shrink-0"
  />
</template>
