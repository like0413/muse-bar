<script setup lang="ts">
import { onMounted, ref } from 'vue'

import { getMediaSessionSourceAppIds } from '@/lib/media-api'

const mediaSessionStatus = ref('正在读取媒体会话')

/** 从 Rust 读取会话 Source App ID，并转换为当前验证页面的单行文本。 */
async function loadMediaSessionSourceAppIds() {
  try {
    const sourceAppIds = await getMediaSessionSourceAppIds()
    mediaSessionStatus.value = sourceAppIds.length
      ? `媒体会话 ${sourceAppIds.length} 个：${sourceAppIds.join('、')}`
      : '未检测到媒体会话'
  } catch {
    mediaSessionStatus.value = '媒体会话读取失败'
  }
}

onMounted(loadMediaSessionSourceAppIds)
</script>

<template>
  <main class="flex h-screen w-screen items-center justify-center bg-transparent p-1">
    <section
      aria-label="Muse Bar"
      class="bg-secondary text-secondary-foreground flex h-full w-full items-center justify-center rounded-md border px-3 text-sm font-medium"
    >
      <span class="truncate" :title="mediaSessionStatus">{{ mediaSessionStatus }}</span>
    </section>
  </main>
</template>
