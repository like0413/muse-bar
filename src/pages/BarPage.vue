<script setup lang="ts">
import { ref } from 'vue'

import { Button } from '@/components/ui/button'
import { openSettingsWindow } from '@/lib/settings-window'
import { readCurrentWindowLabel } from '@/lib/window-label'

const windowLabel = readCurrentWindowLabel()
const isOpeningSettings = ref(false)
const settingsError = ref<string>()

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
</script>

<template>
  <main class="flex min-h-screen items-center gap-3 p-3">
    <div class="flex min-w-0 flex-1 flex-col gap-1">
      <p class="text-muted-foreground text-sm">Taskbar surface</p>
      <h1 class="text-lg font-semibold">Muse Bar</h1>
      <p class="text-sm">
        Window label: <code>{{ windowLabel }}</code>
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
