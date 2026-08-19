<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import {
  getCurrentMediaSnapshot,
  listenToCurrentMediaSnapshotChanges,
  listenToCurrentTimelineChanges,
  type MediaSnapshot,
} from '@/lib/media-api'
import { getRuntimeInfo } from '@/lib/runtime-info'
import {
  getSettings,
  readColorMode,
  readTaskbarPosition,
  updateSettings,
  type ColorMode,
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
const mediaSnapshot = ref<MediaSnapshot | null>(null)
const mediaSnapshotError = ref<string>()
let stopMediaSnapshotListener: UnlistenFn | undefined
let stopTimelineListener: UnlistenFn | undefined
let hasUnmounted = false

const currentPosition = computed(() => readTaskbarPosition(settings.value))
const currentColorMode = computed(() => readColorMode(settings.value))
const mediaSnapshotJson = computed(() => serializeMediaSnapshot(mediaSnapshot.value))

const positionOptions = [
  { value: 'left', label: '靠左' },
  { value: 'center', label: '居中' },
  { value: 'right', label: '靠右' },
] as const

const colorModeOptions: ReadonlyArray<{ value: ColorMode; label: string }> = [
  { value: 'system', label: '跟随系统' },
  { value: 'dark', label: '深色' },
  { value: 'light', label: '浅色' },
]

/** 序列化诊断快照，但不把可能很长的封面 base64 正文插入页面 DOM。 */
function serializeMediaSnapshot(snapshot: MediaSnapshot | null): string {
  if (!snapshot) return 'null'

  const artworkSummary = snapshot.artworkDataUrl
    ? `${snapshot.artworkDataUrl.slice(0, snapshot.artworkDataUrl.indexOf(',') + 1)}<已省略，共 ${snapshot.artworkDataUrl.length} 字符>`
    : null

  return JSON.stringify({ ...snapshot, artworkDataUrl: artworkSummary }, null, 2)
}

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

/** 只替换颜色模式字段，并让 Rust 保存后向所有窗口广播新设置。 */
async function handleColorModeChange(colorMode: unknown): Promise<void> {
  if (
    !settings.value ||
    typeof colorMode !== 'string' ||
    !colorMode ||
    colorMode === currentColorMode.value
  )
    return

  isSavingSettings.value = true
  settingsError.value = undefined

  try {
    settings.value = await updateSettings({ ...settings.value, colorMode })
  } catch (error) {
    settingsError.value = error instanceof Error ? error.message : String(error)
  } finally {
    isSavingSettings.value = false
  }
}

/** 订阅统一媒体事件并读取初始快照，供当前诊断页面直接检查 JSON。 */
async function startMediaSnapshotListener(): Promise<void> {
  try {
    const stopListener = await listenToCurrentMediaSnapshotChanges((snapshot) => {
      mediaSnapshot.value = snapshot
      mediaSnapshotError.value = undefined
    })
    if (hasUnmounted) {
      stopListener()
      return
    }

    stopMediaSnapshotListener = stopListener
    // 时间轴使用轻量事件局部更新，避免仅因位置变化就再次传输封面 data URL。
    const stopTimeline = await listenToCurrentTimelineChanges((timeline) => {
      if (mediaSnapshot.value) mediaSnapshot.value = { ...mediaSnapshot.value, timeline }
    })
    if (hasUnmounted) {
      stopTimeline()
      return
    }

    stopTimelineListener = stopTimeline
    mediaSnapshot.value = await getCurrentMediaSnapshot()
  } catch (error) {
    mediaSnapshotError.value = error instanceof Error ? error.message : String(error)
  }
}

onMounted(() => {
  void loadRuntimeInfo()
  void loadSettings()
  void startMediaSnapshotListener()
})

onBeforeUnmount(() => {
  hasUnmounted = true
  stopMediaSnapshotListener?.()
  stopTimelineListener?.()
})
</script>

<template>
  <main class="bg-background flex min-h-screen flex-col justify-center gap-1 p-6">
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
    <section aria-labelledby="color-mode-heading" class="mt-4 flex flex-col gap-2">
      <h2 id="color-mode-heading" class="text-lg font-medium">颜色模式</h2>
      <ToggleGroup
        type="single"
        variant="outline"
        :disabled="isSavingSettings || !settings"
        :model-value="currentColorMode"
        @update:model-value="handleColorModeChange"
      >
        <ToggleGroupItem
          v-for="option in colorModeOptions"
          :key="option.value"
          :value="option.value"
          :aria-label="option.label"
        >
          {{ option.label }}
        </ToggleGroupItem>
      </ToggleGroup>
      <p class="text-muted-foreground text-sm">当前值：{{ currentColorMode }}</p>
    </section>
    <section aria-labelledby="media-snapshot-heading" class="mt-4 flex flex-col gap-2">
      <h2 id="media-snapshot-heading" class="text-lg font-medium">MediaSnapshot</h2>
      <p v-if="mediaSnapshotError" role="alert" class="text-destructive text-sm">
        {{ mediaSnapshotError }}
      </p>
      <pre
        v-else
        class="bg-muted max-h-72 overflow-auto rounded-md border p-3 text-xs whitespace-pre-wrap"
        >{{ mediaSnapshotJson }}</pre>
    </section>
  </main>
</template>
