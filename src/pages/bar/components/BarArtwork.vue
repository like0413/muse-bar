<script setup lang="ts">
import { Music2Icon } from '@lucide/vue'
import { storeToRefs } from 'pinia'

import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'

import { useBarStore } from '../bar-store'

const barStore = useBarStore()
const { snapshot } = storeToRefs(barStore)
</script>

<template>
  <Avatar class="relative z-10 size-9.5 rounded-sm">
    <AvatarImage
      v-if="snapshot?.artworkDataUrl"
      :src="snapshot.artworkDataUrl"
      alt=""
      class="object-cover"
    />
    <AvatarFallback class="rounded-md" aria-label="暂无歌曲封面">
      <Music2Icon class="text-muted-foreground size-4" />
    </AvatarFallback>
  </Avatar>
</template>
