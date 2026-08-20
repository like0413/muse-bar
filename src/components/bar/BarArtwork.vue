<script setup lang="ts">
import { Music2Icon } from '@lucide/vue'

import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'

defineProps<{
  artworkDataUrl: string | null
}>()
</script>

<template>
  <Avatar class="relative z-10 size-9.5 rounded-sm">
    <AvatarImage v-if="artworkDataUrl" :src="artworkDataUrl" alt="" class="object-cover" />
    <AvatarFallback class="rounded-md" aria-label="暂无歌曲封面">
      <Music2Icon class="text-muted-foreground size-4" />
    </AvatarFallback>
  </Avatar>
</template>
