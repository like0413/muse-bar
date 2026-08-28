<script setup lang="ts">
import { Music2Icon, Volume2Icon } from '@lucide/vue'
import { usePreferredReducedMotion } from '@vueuse/core'
import { computed } from 'vue'

import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Separator } from '@/components/ui/separator'
import type { MediaSnapshot } from '@/lib/media-types'
import type {
  ArtworkShape,
  ElementAlignment,
  ProgressStyle,
  SpectrumOrigin,
} from '@/lib/settings-api'
import { cn } from '@/lib/utils'

interface Props {
  mediaSnapshot: MediaSnapshot | null
  artworkShape: ArtworkShape
  rotateCircularArtwork: boolean
  accentColor: string
  alignment: ElementAlignment
  showControls: boolean
  showProgress: boolean
  spectrumEnabled: boolean
  spectrumOrigin: SpectrumOrigin
  progressStyle: ProgressStyle
}

const props = defineProps<Props>()
const preferredReducedMotion = usePreferredReducedMotion()
const layoutClass = computed(() =>
  cn(
    'bg-card text-card-foreground relative flex h-14 w-full max-w-md items-center overflow-hidden rounded-xl border px-3 shadow-sm',
    { 'flex-row-reverse': props.alignment === 'right' },
  ),
)
const textClass = computed(() =>
  cn('relative min-w-0 flex-1', { 'text-right': props.alignment === 'right' }),
)
const contentClass = computed(() =>
  cn('relative flex h-full min-w-0 flex-1 items-center gap-3 overflow-hidden', {
    'pr-3': props.alignment === 'left',
    'flex-row-reverse pl-3': props.alignment === 'right',
  }),
)
const spectrumRegionClass = computed(() =>
  cn('flex h-full shrink-0 items-center gap-3', {
    'flex-row-reverse': props.alignment === 'right',
  }),
)
const spectrumClass = computed(() =>
  cn('pointer-events-none flex h-full w-16 shrink-0 justify-center gap-1 opacity-90', {
    'items-end': props.spectrumOrigin === 'bottom',
    'items-center': props.spectrumOrigin === 'center',
  }),
)
const artworkClass = computed(() => {
  const rotationConfigured =
    props.artworkShape === 'circle' &&
    props.rotateCircularArtwork &&
    Boolean(props.mediaSnapshot?.artworkDataUrl) &&
    preferredReducedMotion.value !== 'reduce'
  return cn('relative size-10', props.artworkShape === 'circle' ? 'rounded-full' : 'rounded-md', {
    'animate-spin [animation-duration:12s]': rotationConfigured,
    '[animation-play-state:paused]':
      rotationConfigured && props.mediaSnapshot?.playbackStatus !== 'playing',
  })
})
</script>

<template>
  <div class="bg-muted flex min-h-36 items-center justify-center rounded-xl border p-6">
    <div :class="layoutClass">
      <div :class="contentClass">
        <div
          v-if="showProgress && progressStyle === 'background-gradient'"
          class="pointer-events-none absolute inset-y-0 left-0 w-3/5"
          :style="{
            background: `linear-gradient(90deg, transparent, color-mix(in srgb, ${accentColor} 42%, transparent))`,
          }"
        />
        <Avatar :class="artworkClass">
          <AvatarImage
            v-if="mediaSnapshot?.artworkDataUrl"
            :src="mediaSnapshot.artworkDataUrl"
            :alt="mediaSnapshot.title"
          />
          <AvatarFallback><Music2Icon /></AvatarFallback>
        </Avatar>
        <div :class="textClass">
          <p class="truncate text-sm font-medium">
            {{ mediaSnapshot?.title || 'Muse Bar 预览' }}
          </p>
          <p class="text-muted-foreground truncate text-xs">
            {{ mediaSnapshot?.artist || '当前歌曲歌手' }}
          </p>
        </div>
        <div
          v-if="showControls"
          class="relative flex shrink-0 items-center gap-2 text-sm"
          aria-hidden="true"
        >
          <span>◀　Ⅱ　▶</span><Volume2Icon class="size-4" />
        </div>
        <div
          v-if="showProgress && progressStyle === 'underline'"
          class="absolute bottom-0 left-0 h-0.5 w-3/5"
          :style="{ backgroundColor: accentColor }"
        />
      </div>
      <div :class="spectrumRegionClass">
        <Separator orientation="vertical" class="data-[orientation=vertical]:h-5" />
        <div v-if="spectrumEnabled" :class="spectrumClass" aria-hidden="true">
          <span
            v-for="(height, index) in [
              22, 38, 54, 34, 72, 46, 80, 58, 36, 66, 48, 28, 42, 62, 34, 20,
            ]"
            :key="index"
            class="min-w-0 flex-1 rounded-full"
            :style="{ backgroundColor: accentColor, height: `${height}%` }"
          />
        </div>
      </div>
    </div>
  </div>
</template>
