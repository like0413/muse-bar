<script setup lang="ts">
import { onMounted, ref } from 'vue'

import { getRuntimeInfo } from '@/lib/runtime-info'
import { readCurrentWindowLabel } from '@/lib/window-label'
import type { RuntimeInfo } from '@/types/runtime-info'

const windowLabel = readCurrentWindowLabel()
const runtimeInfo = ref<RuntimeInfo>()
const runtimeError = ref<string>()

/** 将 Rust 返回的 Unix 毫秒时间戳转换为本地可读时间。 */
function formatStartedAt(startedAtUnixMs: number): string {
  return new Date(startedAtUnixMs).toLocaleString()
}

/** 加载共享运行状态，并保留可直接诊断的命令错误。 */
async function loadRuntimeInfo(): Promise<void> {
  try {
    runtimeInfo.value = await getRuntimeInfo()
  } catch (error) {
    runtimeError.value = error instanceof Error ? error.message : String(error)
  }
}

onMounted(() => {
  void loadRuntimeInfo()
})
</script>

<template>
  <main class="flex min-h-screen flex-col justify-center gap-1 p-6">
    <p class="text-muted-foreground text-sm">Configuration surface</p>
    <h1 class="text-2xl font-semibold">Muse Bar Settings</h1>
    <p class="text-sm">
      Window label: <code>{{ windowLabel }}</code>
    </p>
    <section aria-labelledby="runtime-heading" class="mt-4 flex flex-col gap-2">
      <h2 id="runtime-heading" class="text-lg font-medium">Rust runtime info</h2>
      <dl v-if="runtimeInfo" class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
        <dt class="text-muted-foreground">Version</dt>
        <dd>{{ runtimeInfo.applicationVersion }}</dd>
        <dt class="text-muted-foreground">Started at</dt>
        <dd>{{ formatStartedAt(runtimeInfo.startedAtUnixMs) }}</dd>
      </dl>
      <p v-else-if="runtimeError" role="alert" class="text-destructive text-sm">
        {{ runtimeError }}
      </p>
      <p v-else class="text-muted-foreground text-sm">Loading from Rust…</p>
    </section>
  </main>
</template>
