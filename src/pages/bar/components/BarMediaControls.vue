<script setup lang="ts">
import {
  PauseIcon,
  PlayIcon,
  SkipBackIcon,
  SkipForwardIcon,
  VolumeIcon,
  Volume1Icon,
  Volume2Icon,
  VolumeXIcon,
} from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed, onMounted, shallowRef, useTemplateRef, type ComponentPublicInstance } from 'vue'

import { Button } from '@/components/ui/button'
import type { ApplicationVolumeState, VolumeFlyoutAnchor } from '@/lib/application-volume-api'
import { controlMedia } from '@/lib/media-control-api'
import type { ControlAction } from '@/lib/media-types'
import { getErrorMessage } from '@/lib/utils'

import { useBarStore } from '../bar-store'

const barStore = useBarStore()
const props = defineProps<{
  volumeState: ApplicationVolumeState | null
  volumeUnavailableReason: string
  volumePending: boolean
}>()
const emit = defineEmits<{
  volumeAnchorChanged: [anchor: VolumeFlyoutAnchor]
  volumeAnchorEntered: []
  volumeAnchorLeft: []
  toggleVolumeMute: []
}>()
const { snapshot } = storeToRefs(barStore)
const volumeButton = useTemplateRef<ComponentPublicInstance>('volumeButton')
const isControlPending = shallowRef(false)
const MEDIA_CONTROL_TIMEOUT_MS = 3_000
const isPlaying = computed(() => snapshot.value?.playbackStatus === 'playing')
const capabilities = computed(() => snapshot.value?.capabilities)
const canTogglePlayPause = computed(() => {
  const currentCapabilities = capabilities.value
  if (!currentCapabilities) return false
  return isPlaying.value ? currentCapabilities.canPause : currentCapabilities.canPlay
})
const volumeLabel = computed(() => {
  if (!props.volumeState) return props.volumeUnavailableReason || '当前应用没有可用的音频会话'
  return props.volumeState.muted
    ? `当前应用音量：${props.volumeState.levelPercent}%（已静音）`
    : `当前应用音量：${props.volumeState.levelPercent}%`
})
const volumeIcon = computed(() => {
  if (!props.volumeState) return VolumeIcon
  if (props.volumeState.muted || props.volumeState.levelPercent === 0) return VolumeXIcon
  return props.volumeState.levelPercent < 50 ? Volume1Icon : Volume2Icon
})

function reportVolumeAnchor(): void {
  const element = volumeButton.value?.$el
  if (!(element instanceof HTMLElement)) return
  const rect = element.getBoundingClientRect()
  emit('volumeAnchorChanged', {
    x: rect.x,
    y: rect.y,
    width: rect.width,
    height: rect.height,
  })
}

function handleVolumeMouseEnter(): void {
  reportVolumeAnchor()
  emit('volumeAnchorEntered')
}

onMounted(reportVolumeAnchor)

/** 阻止同一会话收到并发操作，并把播放器返回的失败原因写入诊断信息。 */
async function performControl(action: ControlAction): Promise<void> {
  if (isControlPending.value) return

  isControlPending.value = true
  barStore.setControlError('')
  let timeout: number | undefined
  try {
    await Promise.race([
      controlMedia(action),
      new Promise<never>((_, reject) => {
        timeout = window.setTimeout(
          () => reject(new Error('播放器响应超时，请稍后重试')),
          MEDIA_CONTROL_TIMEOUT_MS,
        )
      }),
    ])
  } catch (error) {
    barStore.setControlError(`控制失败：${getErrorMessage(error)}`)
  } finally {
    if (timeout !== undefined) window.clearTimeout(timeout)
    isControlPending.value = false
  }
}
</script>

<template>
  <div
    role="group"
    aria-label="媒体控制"
    data-bar-controls
    class="relative z-10 flex shrink-0 items-center gap-0.5"
  >
    <Button
      variant="bar"
      size="icon-sm"
      aria-label="上一曲"
      :disabled="!capabilities?.canPrevious"
      :aria-busy="isControlPending"
      @click="performControl({ type: 'previous' })"
    >
      <SkipBackIcon data-icon="inline-start" fill="currentColor" />
    </Button>
    <Button
      variant="bar"
      size="icon-sm"
      :aria-label="isPlaying ? '暂停' : '播放'"
      :disabled="!canTogglePlayPause"
      :aria-busy="isControlPending"
      @click="performControl({ type: 'togglePlayPause' })"
    >
      <PauseIcon v-if="isPlaying" data-icon="inline-start" fill="currentColor" />
      <PlayIcon v-else data-icon="inline-start" fill="currentColor" />
    </Button>
    <Button
      variant="bar"
      size="icon-sm"
      aria-label="下一曲"
      :disabled="!capabilities?.canNext"
      :aria-busy="isControlPending"
      @click="performControl({ type: 'next' })"
    >
      <SkipForwardIcon data-icon="inline-start" fill="currentColor" />
    </Button>
    <Button
      ref="volumeButton"
      variant="bar"
      size="icon-sm"
      :aria-label="volumeLabel"
      :aria-busy="volumePending"
      @mouseenter="handleVolumeMouseEnter"
      @mouseleave="emit('volumeAnchorLeft')"
      @click="emit('toggleVolumeMute')"
    >
      <component :is="volumeIcon" data-icon="inline-start" />
    </Button>
  </div>
</template>
