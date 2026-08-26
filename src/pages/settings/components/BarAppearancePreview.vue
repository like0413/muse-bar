<script setup lang="ts">
import { Music2Icon } from '@lucide/vue'
import { computed } from 'vue'

import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import type { MediaSnapshot } from '@/lib/media-types'
import type { ElementAlignment, ProgressStyle } from '@/lib/settings-api'
import { cn } from '@/lib/utils'

interface Props {
  mediaSnapshot: MediaSnapshot | null
  accentColor: string
  alignment: ElementAlignment
  showControls: boolean
  showProgress: boolean
  progressStyle: ProgressStyle
}

const props = defineProps<Props>()
const layoutClass = computed(() =>
  cn(
    'bg-card text-card-foreground relative flex h-14 w-full max-w-md items-center gap-3 overflow-hidden rounded-xl border px-3 shadow-sm',
    { 'flex-row-reverse': props.alignment === 'right' },
  ),
)
const textClass = computed(() =>
  cn('relative min-w-0 flex-1', { 'text-right': props.alignment === 'right' }),
)
</script>

<template>
  <div class="bg-muted flex min-h-36 items-center justify-center rounded-xl border p-6">
    <div :class="layoutClass">
      <div
        v-if="showProgress && progressStyle === 'background-gradient'"
        class="pointer-events-none absolute inset-y-0 left-0 w-3/5"
        :style="{
          background: `linear-gradient(90deg, transparent, color-mix(in srgb, ${accentColor} 42%, transparent))`,
        }"
      />
      <Avatar class="relative size-10 rounded-md">
        <AvatarImage
          v-if="mediaSnapshot?.artworkDataUrl"
          :src="mediaSnapshot.artworkDataUrl"
          :alt="mediaSnapshot.title"
        />
        <AvatarFallback class="rounded-md"><Music2Icon /></AvatarFallback>
      </Avatar>
      <div :class="textClass">
        <p class="truncate text-sm font-medium">
          {{ mediaSnapshot?.title || 'Muse Bar 预览' }}
        </p>
        <p class="text-muted-foreground truncate text-xs">
          {{ mediaSnapshot?.artist || '当前歌曲歌手' }}
        </p>
      </div>
      <div v-if="showControls" class="relative shrink-0 text-sm" aria-hidden="true">◀　Ⅱ　▶</div>
      <div
        v-if="showProgress && progressStyle === 'underline'"
        class="absolute bottom-0 left-0 h-0.5 w-3/5"
        :style="{ backgroundColor: accentColor }"
      />
    </div>
  </div>
</template>
