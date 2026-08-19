<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { getRuntimeInfo } from '@/lib/runtime-info'
import {
  getSettings,
  readTaskbarPosition,
  updateSettings,
  type SettingsPayload,
} from '@/lib/settings-api'
import { readCurrentWindowLabel } from '@/lib/window-label'
import type { RuntimeInfo } from '@/types/runtime-info'

const windowLabel = readCurrentWindowLabel()
const runtimeInfo = ref<RuntimeInfo>()
const runtimeError = ref<string>()
const settings = ref<SettingsPayload>()
const settingsError = ref<string>()
const isSavingSettings = ref(false)

const currentPosition = computed(() => readTaskbarPosition(settings.value))

const positionOptions = [
  { value: 'left', label: '靠左' },
  { value: 'center', label: '居中' },
  { value: 'right', label: '靠右' },
] as const

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

/** 读取 Rust 持有的完整设置，作为后续更新的基础数据。 */
async function loadSettings(): Promise<void> {
  try {
    settings.value = await getSettings()
  } catch (error) {
    settingsError.value = error instanceof Error ? error.message : String(error)
  }
}

/** 只替换位置字段，并将其余设置原样交还 Rust 保存。 */
async function handlePositionChange(position: unknown): Promise<void> {
  if (
    !settings.value ||
    typeof position !== 'string' ||
    !position ||
    position === currentPosition.value
  )
    return

  isSavingSettings.value = true
  settingsError.value = undefined

  try {
    settings.value = await updateSettings({ ...settings.value, position })
  } catch (error) {
    settingsError.value = error instanceof Error ? error.message : String(error)
  } finally {
    isSavingSettings.value = false
  }
}

onMounted(() => {
  void loadRuntimeInfo()
  void loadSettings()
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
    <section aria-labelledby="settings-heading" class="mt-4 flex flex-col gap-2">
      <h2 id="settings-heading" class="text-lg font-medium">Taskbar position</h2>
      <ToggleGroup
        type="single"
        variant="outline"
        :disabled="isSavingSettings || !settings"
        :model-value="currentPosition"
        @update:model-value="handlePositionChange"
      >
        <ToggleGroupItem
          v-for="option in positionOptions"
          :key="option.value"
          :value="option.value"
          :aria-label="option.label"
        >
          {{ option.label }}
        </ToggleGroupItem>
      </ToggleGroup>
      <p v-if="settingsError" role="alert" class="text-destructive text-sm">
        {{ settingsError }}
      </p>
      <p v-else class="text-muted-foreground text-sm">
        Current value: {{ currentPosition ?? 'Loading…' }}
      </p>
    </section>
  </main>
</template>
