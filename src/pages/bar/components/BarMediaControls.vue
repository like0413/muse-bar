<script setup lang="ts">
import { PauseIcon, PlayIcon, SkipBackIcon, SkipForwardIcon } from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed, shallowRef } from 'vue'

import { Button } from '@/components/ui/button'
import { ButtonGroup } from '@/components/ui/button-group'
import { controlMedia, type ControlAction } from '@/lib/media-api'

import { useBarStore } from '../bar-store'

const barStore = useBarStore()
const { snapshot } = storeToRefs(barStore)
const isControlPending = shallowRef(false)
const isPlaying = computed(() => snapshot.value?.playbackStatus === 'playing')
const capabilities = computed(() => snapshot.value?.capabilities)
const canTogglePlayPause = computed(() => {
  const currentCapabilities = capabilities.value
  if (!currentCapabilities) return false
  return isPlaying.value ? currentCapabilities.canPause : currentCapabilities.canPlay
})

/** 从 Tauri 的未知拒绝值中提取可读的结构化控制错误。 */
function readControlErrorMessage(error: unknown): string {
  if (typeof error === 'object' && error && 'message' in error) return String(error.message)
  return String(error)
}

/** 阻止同一会话收到并发操作，并把播放器返回的失败原因写入诊断信息。 */
async function performControl(action: ControlAction): Promise<void> {
  if (isControlPending.value) return

  isControlPending.value = true
  barStore.setControlError('')
  try {
    await controlMedia(action)
  } catch (error) {
    barStore.setControlError(`控制失败：${readControlErrorMessage(error)}`)
  } finally {
    isControlPending.value = false
  }
}
</script>

<template>
  <ButtonGroup aria-label="媒体控制" class="relative z-10 shrink-0">
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label="上一曲"
      title="上一曲"
      :disabled="isControlPending || !capabilities?.canPrevious"
      @click="performControl({ type: 'previous' })"
    >
      <SkipBackIcon data-icon="inline-start" />
    </Button>
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
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label="下一曲"
      title="下一曲"
      :disabled="isControlPending || !capabilities?.canNext"
      @click="performControl({ type: 'next' })"
    >
      <SkipForwardIcon data-icon="inline-start" />
    </Button>
  </ButtonGroup>
</template>
