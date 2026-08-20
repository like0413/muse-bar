<script setup lang="ts">
defineProps<{
  title: string
  artist: string
  details: string
}>()
</script>

<template>
  <div class="relative z-10 flex min-w-0 flex-1 flex-col justify-center" :title="details">
    <p data-bar-title class="truncate text-sm font-medium">{{ title }}</p>
    <p v-if="artist" data-bar-artist class="text-muted-foreground truncate text-xs font-normal">
      {{ artist }}
    </p>
  </div>
</template>
