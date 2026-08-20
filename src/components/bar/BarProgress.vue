<script setup lang="ts">
import { computed } from 'vue'

import type { ProgressStyle } from '@/lib/settings-api'

const props = defineProps<{
  progressStyle: ProgressStyle
  percentage: number
  accentColor: string
}>()

/** 将十六进制主色转换为带透明度的颜色，供渐变背景复用。 */
function accentWithAlpha(color: string, alpha: number): string {
  const hex = /^#([0-9a-f]{6})$/i.exec(color)?.[1] ?? '0078D4'
  const red = Number.parseInt(hex.slice(0, 2), 16)
  const green = Number.parseInt(hex.slice(2, 4), 16)
  const blue = Number.parseInt(hex.slice(4, 6), 16)
  return `rgba(${red}, ${green}, ${blue}, ${alpha})`
}

const backgroundProgressStyle = computed(() => ({
  width: `${props.percentage}%`,
  backgroundImage: `linear-gradient(90deg, ${accentWithAlpha(props.accentColor, 0.06)}, ${accentWithAlpha(props.accentColor, 0.32)})`,
}))
</script>

<template>
  <span
    v-if="progressStyle === 'background-gradient'"
    aria-hidden="true"
    class="pointer-events-none absolute inset-y-0 left-0 z-0 transition-[width] duration-150"
    :style="backgroundProgressStyle"
  />
  <span
    v-if="progressStyle === 'underline'"
    aria-hidden="true"
    class="pointer-events-none absolute bottom-0 left-0 z-20 h-0.5 transition-[width] duration-150"
    :style="{ backgroundColor: accentColor, width: `${percentage}%` }"
  />
</template>
