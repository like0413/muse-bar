<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { computed } from 'vue'

import { readLyricsAlignment } from '@/lib/settings-api'
import { cn } from '@/lib/utils'

import { useBarStore } from '../bar-store'

const PLACEHOLDER_LYRICS = '这是一句十二字占位歌词呀'
const barStore = useBarStore()
const { settings } = storeToRefs(barStore)
const lyricsAlignment = computed(() => readLyricsAlignment(settings.value))
const lyricsClass = computed(() =>
  cn('w-full truncate text-sm font-medium', {
    'text-left': lyricsAlignment.value === 'left',
    'text-center': lyricsAlignment.value === 'center',
    'text-right': lyricsAlignment.value === 'right',
  }),
)
</script>

<template>
  <p :class="lyricsClass">{{ PLACEHOLDER_LYRICS }}</p>
</template>
