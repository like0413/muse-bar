<script setup lang="ts">
import { Music2Icon } from '@lucide/vue'
import { usePreferredReducedMotion } from '@vueuse/core'
import { storeToRefs } from 'pinia'
import { computed } from 'vue'

import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { readArtworkShape, readRotateCircularArtwork } from '@/lib/settings-api'
import { cn } from '@/lib/utils'

import { useBarStore } from '../bar-store'

const barStore = useBarStore()
const { settings, snapshot } = storeToRefs(barStore)
const preferredReducedMotion = usePreferredReducedMotion()
const artworkShape = computed(() => readArtworkShape(settings.value))
const rotationConfigured = computed(
  () =>
    artworkShape.value === 'circle' &&
    readRotateCircularArtwork(settings.value) &&
    Boolean(snapshot.value?.artworkDataUrl) &&
    preferredReducedMotion.value !== 'reduce',
)
const artworkClass = computed(() =>
  cn('relative z-10 size-9', artworkShape.value === 'circle' ? 'rounded-full' : 'rounded-sm', {
    'animate-spin [animation-duration:12s]': rotationConfigured.value,
    '[animation-play-state:paused]':
      rotationConfigured.value && snapshot.value?.playbackStatus !== 'playing',
  }),
)
</script>

<template>
  <Avatar :class="artworkClass">
    <AvatarImage
      v-if="snapshot?.artworkDataUrl"
      :src="snapshot.artworkDataUrl"
      alt=""
      class="object-cover"
    />
    <AvatarFallback aria-label="暂无歌曲封面">
      <Music2Icon class="text-muted-foreground size-4" />
    </AvatarFallback>
  </Avatar>
</template>
