<script setup lang="ts">
import { onBeforeUnmount, onMounted } from 'vue'

import { startColorModeSync } from '@/lib/color-mode'

let stopColorModeSync: (() => void) | undefined
let hasUnmounted = false

/** 在当前 WebView 挂载后启动颜色同步，并处理异步初始化期间卸载的情况。 */
async function initializeColorMode(): Promise<void> {
  const stopSync = await startColorModeSync()
  if (hasUnmounted) {
    stopSync()
    return
  }

  stopColorModeSync = stopSync
}

onMounted(() => void initializeColorMode())
onBeforeUnmount(() => {
  hasUnmounted = true
  stopColorModeSync?.()
})
</script>

<template>
  <RouterView />
</template>
