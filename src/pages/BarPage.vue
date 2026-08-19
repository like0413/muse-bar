<script setup lang="ts">
import { Music2Icon, PauseIcon, PlayIcon, SkipBackIcon, SkipForwardIcon } from '@lucide/vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'

import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Button } from '@/components/ui/button'
import { ButtonGroup, ButtonGroupSeparator } from '@/components/ui/button-group'
import { reportBarContentWidth } from '@/lib/bar-layout-api'
import {
  controlMedia,
  getCurrentMediaMetadata,
  getCurrentPlaybackCapabilities,
  getCurrentPlaybackStatus,
  getCurrentTimeline,
  listenToMediaSessionActivityChanges,
  listenToCurrentMediaMetadataChanges,
  listenToCurrentPlaybackCapabilitiesChanges,
  listenToCurrentPlaybackStatusChanges,
  listenToCurrentTimelineChanges,
  refreshSelectedMediaSession,
  type CurrentMediaMetadata,
  type CurrentPlaybackCapabilities,
  type CurrentPlaybackStatus,
  type CurrentTimeline,
  type ControlAction,
  type MediaSelectionReason,
} from '@/lib/media-api'
import { listenToSettingsChanges } from '@/lib/settings-api'
import { openSettingsWindow } from '@/lib/settings-window'

const mediaMetadataStatus = ref('正在读取媒体信息')
const mediaMetadataDetails = ref('')
const currentTitle = ref('')
const currentArtist = ref('')
const artworkDataUrl = ref<string | null>(null)
const accentColor = ref('#0078D4')
const playbackStatusText = ref('')
const playbackCapabilitiesText = ref('')
const timelineDetails = ref('')
const settingsWindowError = ref('')
const mediaSelectionText = ref('')
const controlError = ref('')
const barWidthDetails = ref('宽度：等待测量')
const isControlPending = ref(false)
const currentPlaybackStatus = ref<CurrentPlaybackStatus | null>(null)
const currentPlaybackCapabilities = ref<CurrentPlaybackCapabilities | null>(null)
const currentTimeline = ref<CurrentTimeline | null>(null)
const barPageElement = ref<HTMLElement>()
const barSurfaceElement = ref<HTMLElement>()
const titleElement = ref<HTMLElement>()
const artistElement = ref<HTMLElement>()
let stopMediaMetadataListener: UnlistenFn | undefined
let stopPlaybackCapabilitiesListener: UnlistenFn | undefined
let stopPlaybackStatusListener: UnlistenFn | undefined
let stopTimelineListener: UnlistenFn | undefined
let stopMediaActivityListener: UnlistenFn | undefined
let stopSettingsListener: UnlistenFn | undefined
let hasUnmounted = false
let barResizeObserver: ResizeObserver | undefined
let barMeasurementFrame: number | undefined
let lastReportedNaturalWidth = 0

const mediaDetails = computed(() =>
  [
    mediaMetadataDetails.value,
    playbackStatusText.value && `播放状态：${playbackStatusText.value}`,
    timelineDetails.value,
    playbackCapabilitiesText.value && `控制能力：${playbackCapabilitiesText.value}`,
    mediaSelectionText.value,
    barWidthDetails.value,
    settingsWindowError.value,
    controlError.value,
  ]
    .filter(Boolean)
    .join('\n'),
)

const displayTitle = computed(() => currentTitle.value || mediaMetadataStatus.value)

const playbackStatusLabels: Record<CurrentPlaybackStatus, string> = {
  closed: '已关闭',
  opened: '已打开',
  changing: '切换中',
  stopped: '已停止',
  playing: '播放中',
  paused: '已暂停',
  unknown: '状态未知',
}

const selectionReasonLabels: Record<MediaSelectionReason, string> = {
  playingPreferred: '最近播放的音乐播放器',
  lastPausedPreferred: '最近暂停的音乐播放器',
  windowsCurrentFallback: 'Windows 当前媒体',
}

const isPlaying = computed(() => currentPlaybackStatus.value === 'playing')
const canTogglePlayPause = computed(() => {
  const capabilities = currentPlaybackCapabilities.value
  if (!capabilities) return false
  return isPlaying.value ? capabilities.canPause : capabilities.canPlay
})
const progressPercentage = computed(() => {
  const timeline = currentTimeline.value
  if (!timeline) return 0

  const duration = timeline.endMs - timeline.startMs
  if (duration <= 0) return 0

  const elapsed = timeline.positionMs - timeline.startMs
  return Math.min(100, Math.max(0, (elapsed / duration) * 100))
})

/** 将当前会话元数据转换为 Bar 的文本和完整悬停说明。 */
function showCurrentMediaMetadata(metadata: CurrentMediaMetadata | null) {
  if (!metadata) {
    mediaMetadataStatus.value = '当前没有媒体会话'
    mediaMetadataDetails.value = mediaMetadataStatus.value
    currentTitle.value = ''
    currentArtist.value = ''
    artworkDataUrl.value = null
    accentColor.value = '#0078D4'
    void nextTick(scheduleBarMeasurement)
    return
  }

  const title = metadata.title || '未知标题'
  currentTitle.value = title
  currentArtist.value = metadata.artist
  mediaMetadataStatus.value = metadata.artist ? `${title} · ${metadata.artist}` : title
  mediaMetadataDetails.value = `${metadata.sourceAppId}\n标题：${title}\n歌手：${metadata.artist || '未知歌手'}`
  artworkDataUrl.value = metadata.artworkDataUrl
  accentColor.value = metadata.accentColor || '#0078D4'
  void nextTick(scheduleBarMeasurement)
}

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
  if (!element || !element.textContent) return 0

  const range = document.createRange()
  range.selectNodeContents(element)
  return range.getBoundingClientRect().width
}

/** 汇总封面、自然文本、控制按钮、间距和容器边界所需的完整逻辑宽度。 */
function measureBarNaturalWidth(): number | undefined {
  const page = barPageElement.value
  const surface = barSurfaceElement.value
  const title = titleElement.value
  if (!page || !surface || !title) return undefined

  const artwork = surface.querySelector<HTMLElement>('[data-slot="avatar"]')
  const controls = surface.querySelector<HTMLElement>('[data-slot="button-group"]')
  if (!artwork || !controls) return undefined

  const surfaceStyle = window.getComputedStyle(surface)
  const gap = readCssPixels(surfaceStyle.columnGap || surfaceStyle.gap)
  const textWidth = Math.max(
    measureTextContentWidth(title),
    measureTextContentWidth(artistElement.value),
  )

  return (
    readHorizontalInsets(page) +
    readHorizontalInsets(surface) +
    artwork.getBoundingClientRect().width +
    controls.getBoundingClientRect().width +
    textWidth +
    gap * 2
  )
}

/** 在下一帧合并连续的布局变化，并把新的自然宽度上报给 Rust。 */
function scheduleBarMeasurement(): void {
  if (hasUnmounted || barMeasurementFrame !== undefined) return

  barMeasurementFrame = window.requestAnimationFrame(() => {
    barMeasurementFrame = undefined
    const naturalWidth = measureBarNaturalWidth()
    if (naturalWidth === undefined || Math.abs(naturalWidth - lastReportedNaturalWidth) < 0.5) {
      return
    }

    lastReportedNaturalWidth = naturalWidth
    void reportBarContentWidth(naturalWidth)
      .then((measurement) => {
        barWidthDetails.value = `宽度：自然 ${measurement.naturalWidth.toFixed(1)}，目标 ${measurement.targetWidth}，范围 ${measurement.minimumWidth}–${measurement.maximumWidth}`
      })
      .catch((error: unknown) => {
        barWidthDetails.value = `宽度测量上报失败：${error instanceof Error ? error.message : String(error)}`
      })
  })
}

/** 观察 Bar 各区域尺寸，并在字体完成加载后补做一次自然宽度测量。 */
function startBarMeasurement(): void {
  const surface = barSurfaceElement.value
  const title = titleElement.value
  if (!surface || !title) return

  barResizeObserver = new ResizeObserver(scheduleBarMeasurement)
  barResizeObserver.observe(surface)
  barResizeObserver.observe(title)
  if (artistElement.value) barResizeObserver.observe(artistElement.value)
  scheduleBarMeasurement()

  void document.fonts.ready.then(() => {
    if (!hasUnmounted) scheduleBarMeasurement()
  })
}

/** 设置边界变化时强制重新上报同一自然宽度，让 Rust 计算新的限制结果。 */
async function startBarSettingsListener(): Promise<void> {
  try {
    const stopListener = await listenToSettingsChanges(() => {
      lastReportedNaturalWidth = 0
      scheduleBarMeasurement()
    })
    if (hasUnmounted) {
      stopListener()
      return
    }

    stopSettingsListener = stopListener
  } catch (error) {
    barWidthDetails.value = `宽度设置监听失败：${error instanceof Error ? error.message : String(error)}`
  }
}

/** 响应 Bar 右键操作，打开已有设置窗口或创建一个新的设置窗口。 */
async function handleOpenSettings(): Promise<void> {
  settingsWindowError.value = ''

  try {
    await openSettingsWindow()
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    settingsWindowError.value = `设置页打开失败：${message}`
  }
}

/** 请求 Rust 应用最新会话选择，并把选择原因加入悬停诊断文本。 */
async function applyLatestMediaSelection(): Promise<void> {
  try {
    const selection = await refreshSelectedMediaSession()
    mediaSelectionText.value = selection
      ? `选择：${selectionReasonLabels[selection.reason]}`
      : '选择：当前没有媒体会话'
  } catch {
    mediaSelectionText.value = '选择：刷新失败'
  }
}

/** 从 Tauri 的未知拒绝值中提取可读的结构化控制错误。 */
function readControlErrorMessage(error: unknown): string {
  if (typeof error === 'object' && error && 'message' in error) {
    return String(error.message)
  }
  return String(error)
}

/** 禁止重复点击期间并发控制同一会话，并显示播放器返回的失败原因。 */
async function performControl(action: ControlAction): Promise<void> {
  if (isControlPending.value) return

  isControlPending.value = true
  controlError.value = ''
  try {
    await controlMedia(action)
  } catch (error) {
    controlError.value = `控制失败：${readControlErrorMessage(error)}`
  } finally {
    isControlPending.value = false
  }
}

/** 将 Windows 播放状态转换为当前验证页面使用的中文文本。 */
function showCurrentPlaybackStatus(status: CurrentPlaybackStatus | null) {
  currentPlaybackStatus.value = status
  playbackStatusText.value = status ? playbackStatusLabels[status] : '无播放状态'
}

/** 将播放器声明为可用的控制能力汇总为单行文本。 */
function showCurrentPlaybackCapabilities(capabilities: CurrentPlaybackCapabilities | null) {
  currentPlaybackCapabilities.value = capabilities
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
  currentTimeline.value = timeline
  if (!timeline) {
    timelineDetails.value = '无有效时间轴'
    return
  }

  const elapsed = timeline.positionMs - timeline.startMs
  const duration = timeline.endMs - timeline.startMs
  const rate = timeline.playbackRate === null ? '速率未知' : `${timeline.playbackRate}x`
  timelineDetails.value = [
    `进度：${formatDuration(elapsed)}/${formatDuration(duration)} · ${rate}`,
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
    timelineDetails.value = '时间轴读取失败'
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
    timelineDetails.value = '时间轴监听失败'
  }
}

/** 只在 Bar 页面监听活动变化，并触发 Rust 重新应用全局播放器选择。 */
async function startMediaSelectionListener(): Promise<void> {
  try {
    const stopListener = await listenToMediaSessionActivityChanges(() => {
      void applyLatestMediaSelection()
    })
    if (hasUnmounted) {
      stopListener()
      return
    }

    stopMediaActivityListener = stopListener
    await applyLatestMediaSelection()
  } catch {
    mediaSelectionText.value = '选择：监听失败'
  }
}

onMounted(startMediaMetadataListener)
onMounted(startPlaybackCapabilitiesListener)
onMounted(startPlaybackStatusListener)
onMounted(startTimelineListener)
onMounted(startMediaSelectionListener)
onMounted(startBarMeasurement)
onMounted(startBarSettingsListener)

onBeforeUnmount(() => {
  hasUnmounted = true
  stopMediaMetadataListener?.()
  stopPlaybackCapabilitiesListener?.()
  stopPlaybackStatusListener?.()
  stopTimelineListener?.()
  stopMediaActivityListener?.()
  stopSettingsListener?.()
  barResizeObserver?.disconnect()
  if (barMeasurementFrame !== undefined) window.cancelAnimationFrame(barMeasurementFrame)
})
</script>

<template>
  <main
    ref="barPageElement"
    class="flex h-screen w-screen items-center justify-center bg-transparent"
  >
    <section
      ref="barSurfaceElement"
      aria-label="Muse Bar"
      class="bg-secondary text-secondary-foreground relative flex h-full w-full items-center gap-2 overflow-hidden border px-2 text-sm font-medium"
      @contextmenu.prevent="handleOpenSettings"
    >
      <Avatar class="size-8 rounded-md border">
        <AvatarImage v-if="artworkDataUrl" :src="artworkDataUrl" alt="" class="object-cover" />
        <AvatarFallback class="rounded-md" aria-label="暂无歌曲封面">
          <Music2Icon class="text-muted-foreground size-4" />
        </AvatarFallback>
      </Avatar>
      <div class="flex min-w-0 flex-1 flex-col justify-center" :title="mediaDetails">
        <p ref="titleElement" class="truncate text-sm font-medium">{{ displayTitle }}</p>
        <p
          v-if="currentArtist"
          ref="artistElement"
          class="text-muted-foreground truncate text-xs font-normal"
        >
          {{ currentArtist }}
        </p>
      </div>
      <ButtonGroup aria-label="媒体控制" class="shrink-0">
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="上一曲"
          title="上一曲"
          :disabled="isControlPending || !currentPlaybackCapabilities?.canPrevious"
          @click="performControl({ type: 'previous' })"
        >
          <SkipBackIcon data-icon="inline-start" />
        </Button>
        <ButtonGroupSeparator />
        <Button
          variant="ghost"
          size="icon-sm"
          :aria-label="isPlaying ? '暂停' : '播放'"
          :title="isPlaying ? '暂停' : '播放'"
          :disabled="isControlPending || !canTogglePlayPause"
          @click="performControl({ type: 'togglePlayPause' })"
        >
          <PauseIcon v-if="isPlaying" data-icon="inline-start" />
          <PlayIcon v-else data-icon="inline-start" />
        </Button>
        <ButtonGroupSeparator />
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="下一曲"
          title="下一曲"
          :disabled="isControlPending || !currentPlaybackCapabilities?.canNext"
          @click="performControl({ type: 'next' })"
        >
          <SkipForwardIcon data-icon="inline-start" />
        </Button>
      </ButtonGroup>
      <span
        aria-hidden="true"
        class="absolute bottom-0 left-0 h-0.5 transition-[width] duration-150"
        :style="{ backgroundColor: accentColor, width: `${progressPercentage}%` }"
      />
    </section>
  </main>
</template>
