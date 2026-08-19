<script setup lang="ts">
import { onMounted, ref } from 'vue'

import { isSystemMediaManagerInitialized } from '@/lib/media-api'

const mediaManagerStatus = ref('正在检查媒体管理器')

/** 从 Rust 查询进程级媒体管理器状态，并转换为当前验证页面的可读文本。 */
async function loadMediaManagerStatus() {
  try {
    const initialized = await isSystemMediaManagerInitialized()
    mediaManagerStatus.value = initialized ? '媒体管理器已初始化' : '媒体管理器不可用'
  } catch {
    mediaManagerStatus.value = '媒体管理器查询失败'
  }
}

onMounted(loadMediaManagerStatus)
</script>

<template>
  <main class="flex h-screen w-screen items-center justify-center bg-transparent p-1">
    <section
      aria-label="Muse Bar"
      class="bg-secondary text-secondary-foreground flex h-full w-full items-center justify-center rounded-md border px-3 text-sm font-medium"
    >
      <span>{{ mediaManagerStatus }}</span>
    </section>
  </main>
</template>
