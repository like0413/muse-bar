<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import { onBeforeUnmount, onMounted, ref } from 'vue'

import { Button } from '@/components/ui/button'
import { getSettings, listenToSettingsChanges, readTaskbarPosition } from '@/lib/settings-api'
import { openSettingsWindow } from '@/lib/settings-window'
import { readCurrentWindowLabel } from '@/lib/window-label'

const windowLabel = readCurrentWindowLabel()
const isOpeningSettings = ref(false)
const settingsError = ref<string>()
const currentPosition = ref<string>()
let unlistenSettings: UnlistenFn | undefined

/** 打开设置窗口，并将原生窗口错误显示在 Bar 中。 */
async function handleOpenSettings(): Promise<void> {
  isOpeningSettings.value = true
  settingsError.value = undefined

  try {
    await openSettingsWindow()
  } catch (error) {
    settingsError.value = error instanceof Error ? error.message : 'Unable to open settings'
  } finally {
    isOpeningSettings.value = false
  }
}

/** 先建立事件监听，再读取当前值，避免 Bar 错过设置页的即时更新。 */
async function startSettingsSync(): Promise<void> {
  try {
    unlistenSettings = await listenToSettingsChanges((settings) => {
      currentPosition.value = readTaskbarPosition(settings)
    })
    currentPosition.value = readTaskbarPosition(await getSettings())
  } catch (error) {
    settingsError.value = error instanceof Error ? error.message : String(error)
  }
}

onMounted(() => {
  void startSettingsSync()
})

onBeforeUnmount(() => {
  unlistenSettings?.()
})
</script>

<template>
  <main class="flex min-h-screen items-center gap-3 p-3">
    <div class="flex min-w-0 flex-1 flex-col gap-1">
      <p class="text-muted-foreground text-sm">Taskbar surface</p>
      <h1 class="text-lg font-semibold">Muse Bar</h1>
      <p class="text-sm">
        Window label: <code>{{ windowLabel }}</code>
      </p>
      <p class="text-sm">
        Taskbar position: <code>{{ currentPosition ?? 'Loading…' }}</code>
      </p>
      <p v-if="settingsError" role="alert" class="text-destructive text-sm">
        {{ settingsError }}
      </p>
    </div>
    <Button
      class="shrink-0"
      size="sm"
      variant="outline"
      :disabled="isOpeningSettings"
      @click="handleOpenSettings"
    >
      {{ isOpeningSettings ? 'Opening…' : 'Open settings' }}
    </Button>
  </main>
</template>
