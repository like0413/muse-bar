<script setup lang="ts">
import { PauseIcon, PlayIcon, SkipBackIcon, SkipForwardIcon } from '@lucide/vue'

import { Button } from '@/components/ui/button'
import { ButtonGroup } from '@/components/ui/button-group'

defineProps<{
  isPlaying: boolean
  isPending: boolean
  canTogglePlayPause: boolean
  canPrevious: boolean
  canNext: boolean
}>()

defineEmits<{
  previous: []
  togglePlayPause: []
  next: []
}>()
</script>

<template>
  <ButtonGroup aria-label="媒体控制" class="relative z-10 shrink-0">
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label="上一曲"
      title="上一曲"
      :disabled="isPending || !canPrevious"
      @click="$emit('previous')"
    >
      <SkipBackIcon data-icon="inline-start" />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      :aria-label="isPlaying ? '暂停' : '播放'"
      :title="isPlaying ? '暂停' : '播放'"
      :disabled="isPending || !canTogglePlayPause"
      @click="$emit('togglePlayPause')"
    >
      <PauseIcon v-if="isPlaying" data-icon="inline-start" />
      <PlayIcon v-else data-icon="inline-start" />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label="下一曲"
      title="下一曲"
      :disabled="isPending || !canNext"
      @click="$emit('next')"
    >
      <SkipForwardIcon data-icon="inline-start" />
    </Button>
  </ButtonGroup>
</template>
