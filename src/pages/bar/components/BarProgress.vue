<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { computed } from 'vue'

import {
  readCustomProgressColor,
  readProgressColorSource,
  readProgressStyle,
  readShowProgress,
} from '@/lib/settings-api'

import { useBarStore } from '../bar-store'

const barStore = useBarStore()
const { settings, snapshot } = storeToRefs(barStore)
const showProgress = computed(() => readShowProgress(settings.value))
const progressStyle = computed(() => readProgressStyle(settings.value))
const accentColor = computed(() => {
  const source = readProgressColorSource(settings.value)
  if (source === 'custom') return readCustomProgressColor(settings.value)
  if (source === 'system') return snapshot.value?.systemAccentColor || '#0078D4'
  return snapshot.value?.accentColor || '#0078D4'
})
const progressPercentage = computed(() => {
  const timeline = snapshot.value?.timeline
  if (!timeline) return 0

  const duration = timeline.endMs - timeline.startMs
  if (duration <= 0) return 0

  const elapsed = timeline.positionMs - timeline.startMs
  return Math.min(100, Math.max(0, (elapsed / duration) * 100))
})

/** 将十六进制主色转换为带透明度的颜色，供渐变背景复用。 */
function accentWithAlpha(color: string, alpha: number): string {
  const hex = /^#([0-9a-f]{6})$/i.exec(color)?.[1] ?? '0078D4'
  const red = Number.parseInt(hex.slice(0, 2), 16)
  const green = Number.parseInt(hex.slice(2, 4), 16)
  const blue = Number.parseInt(hex.slice(4, 6), 16)
  return `rgba(${red}, ${green}, ${blue}, ${alpha})`
}

const backgroundProgressStyle = computed(() => ({
  width: `${progressPercentage.value}%`,
  backgroundImage: `linear-gradient(90deg, ${accentWithAlpha(accentColor.value, 0.06)}, ${accentWithAlpha(accentColor.value, 0.32)})`,
}))
</script>

<template>
  <span
    v-if="showProgress && progressStyle === 'background-gradient'"
    aria-hidden="true"
    class="pointer-events-none absolute inset-y-0 left-0 z-0 transition-[width] duration-150"
    :style="backgroundProgressStyle"
  />
  <span
    v-if="showProgress && progressStyle === 'underline'"
    aria-hidden="true"
    class="pointer-events-none absolute bottom-0 left-0 z-20 h-0.5 transition-[width] duration-150"
    :style="{ backgroundColor: accentColor, width: `${progressPercentage}%` }"
  />
</template>
