<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { Slider } from '@/components/ui/slider'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import {
  controlMedia,
  getCurrentMediaSnapshot,
  getMediaSessionIdentities,
  getMediaSessionActivities,
  listenToCurrentMediaSnapshotChanges,
  listenToCurrentTimelineChanges,
  listenToMediaSessionIdentityChanges,
  listenToMediaSessionActivityChanges,
  type MediaActivityReason,
  type MediaPlayerKind,
  type MediaSessionActivity,
  type MediaSessionIdentity,
  type MediaSnapshot,
} from '@/lib/media-api'
import { getRuntimeInfo } from '@/lib/runtime-info'
import {
  getSettings,
  readColorMode,
  readProgressStyle,
  readTaskbarPosition,
  updateSettings,
  type ColorMode,
  type ProgressStyle,
  type SettingsPayload,
} from '@/lib/settings-api'
import { getTaskbarOccupiedRegions, type TaskbarOccupancy } from '@/lib/taskbar-diagnostics-api'
import { readCurrentWindowLabel } from '@/lib/window-label'
import type { RuntimeInfo } from '@/types/runtime-info'

const windowLabel = readCurrentWindowLabel()
const runtimeInfo = ref<RuntimeInfo>()
const runtimeError = ref<string>()
const settings = ref<SettingsPayload>()
const settingsError = ref<string>()
const isSavingSettings = ref(false)
const taskbarOccupancy = ref<TaskbarOccupancy>()
const taskbarOccupancyError = ref<string>()
const mediaSnapshot = ref<MediaSnapshot | null>(null)
const mediaSnapshotError = ref<string>()
const mediaSessionIdentities = ref<MediaSessionIdentity[]>([])
const mediaSessionActivities = ref<MediaSessionActivity[]>([])
const seekValue = ref<number[]>([0])
const isSeekPreviewing = ref(false)
const isSeekPending = ref(false)
const seekFeedback = ref('')
let stopMediaSnapshotListener: UnlistenFn | undefined
let stopTimelineListener: UnlistenFn | undefined
let stopMediaSessionIdentityListener: UnlistenFn | undefined
let stopMediaSessionActivityListener: UnlistenFn | undefined
let hasUnmounted = false

const currentPosition = computed(() => readTaskbarPosition(settings.value))
const currentColorMode = computed(() => readColorMode(settings.value))
const currentProgressStyle = computed(() => readProgressStyle(settings.value))
const mediaSnapshotJson = computed(() => serializeMediaSnapshot(mediaSnapshot.value))
const seekMinimum = computed(() => mediaSnapshot.value?.timeline?.minSeekMs ?? 0)
const seekMaximum = computed(() => {
  const timeline = mediaSnapshot.value?.timeline
  if (!timeline) return 0
  return timeline.maxSeekMs > timeline.minSeekMs ? timeline.maxSeekMs : timeline.endMs
})
const canSeek = computed(
  () => Boolean(mediaSnapshot.value?.capabilities.canSeek) && seekMaximum.value > seekMinimum.value,
)

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

const progressStyleOptions: ReadonlyArray<{ value: ProgressStyle; label: string }> = [
  { value: 'underline', label: '底部细线' },
  { value: 'background-gradient', label: '背景渐变' },
]

const playerKindLabels: Record<MediaPlayerKind, string> = {
  qqMusic: 'QQ 音乐',
  neteaseCloudMusic: '网易云音乐',
  kugouMusic: '酷狗音乐',
  qishuiMusic: '汽水音乐',
  other: '普通系统媒体',
}

const activityReasonLabels: Record<MediaActivityReason, string> = {
  detectedPlaying: '启动时已在播放',
  playbackStarted: '开始播放',
  trackChanged: '切歌',
  becameCurrent: '成为系统当前会话',
}

const taskbarOccupancySourceLabels = {
  uiAutomation: 'UI Automation',
  win32Fallback: 'Win32 子窗口回退',
} as const

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

/** 将毫秒格式化为设置页验证 seek 时易读的时长。 */
function formatDuration(milliseconds: number): string {
  const safeMilliseconds = Math.max(0, milliseconds)
  const totalSeconds = Math.floor(safeMilliseconds / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
}

/** 从 Tauri 的未知拒绝值中提取可读的结构化控制错误。 */
function readControlErrorMessage(error: unknown): string {
  if (typeof error === 'object' && error && 'message' in error) return String(error.message)
  return String(error)
}

/** 记录滑块预览位置；拖动期间不让播放器的时间轴事件把手柄拉回去。 */
function handleSeekPreview(value: number[]): void {
  seekValue.value = value
  isSeekPreviewing.value = true
  seekFeedback.value = ''
}

/** 用户释放滑块后只提交一次 seek，避免拖动时连续调用播放器接口。 */
async function commitSeek(value: number[]): Promise<void> {
  isSeekPreviewing.value = false
  const positionMs = value[0]
  if (!canSeek.value || positionMs === undefined || isSeekPending.value) return

  isSeekPending.value = true
  seekFeedback.value = ''
  try {
    await controlMedia({ type: 'seek', positionMs: Math.round(positionMs) })
    seekFeedback.value = `已请求跳转到 ${formatDuration(positionMs)}`
  } catch (error) {
    seekFeedback.value = `跳转失败：${readControlErrorMessage(error)}`
  } finally {
    isSeekPending.value = false
  }
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

/** 读取任务栏原生控件占用矩形，供 9.4 在设置页直接验证。 */
async function loadTaskbarOccupancy(): Promise<void> {
  try {
    taskbarOccupancy.value = await getTaskbarOccupiedRegions()
    taskbarOccupancyError.value = undefined
  } catch (error) {
    taskbarOccupancyError.value = error instanceof Error ? error.message : String(error)
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

/** 只替换进度样式字段，保存后由设置事件实时更新 Bar。 */
async function handleProgressStyleChange(progressStyle: unknown): Promise<void> {
  if (
    !settings.value ||
    typeof progressStyle !== 'string' ||
    !progressStyle ||
    progressStyle === currentProgressStyle.value
  )
    return

  isSavingSettings.value = true
  settingsError.value = undefined

  try {
    settings.value = await updateSettings({ ...settings.value, progressStyle })
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

/** 订阅会话列表变化，并读取首次打开设置页时已经存在的播放器身份。 */
async function startMediaSessionIdentityListener(): Promise<void> {
  try {
    const stopListener = await listenToMediaSessionIdentityChanges((identities) => {
      mediaSessionIdentities.value = identities
    })
    if (hasUnmounted) {
      stopListener()
      return
    }

    stopMediaSessionIdentityListener = stopListener
    mediaSessionIdentities.value = await getMediaSessionIdentities()
  } catch (error) {
    mediaSnapshotError.value = error instanceof Error ? error.message : String(error)
  }
}

/** 订阅所有会话的活动记录，用递增序号直观验证时间轴更新不会抢占活跃度。 */
async function startMediaSessionActivityListener(): Promise<void> {
  try {
    const stopListener = await listenToMediaSessionActivityChanges((activities) => {
      mediaSessionActivities.value = activities
    })
    if (hasUnmounted) {
      stopListener()
      return
    }

    stopMediaSessionActivityListener = stopListener
    mediaSessionActivities.value = await getMediaSessionActivities()
  } catch (error) {
    mediaSnapshotError.value = error instanceof Error ? error.message : String(error)
  }
}

onMounted(() => {
  void loadRuntimeInfo()
  void loadSettings()
  void loadTaskbarOccupancy()
  void startMediaSnapshotListener()
  void startMediaSessionIdentityListener()
  void startMediaSessionActivityListener()
})

watch(
  () => [mediaSnapshot.value?.sessionKey, mediaSnapshot.value?.timeline?.positionMs] as const,
  ([, positionMs]) => {
    if (!isSeekPreviewing.value) seekValue.value = [positionMs ?? 0]
  },
  { immediate: true },
)

onBeforeUnmount(() => {
  hasUnmounted = true
  stopMediaSnapshotListener?.()
  stopTimelineListener?.()
  stopMediaSessionIdentityListener?.()
  stopMediaSessionActivityListener?.()
})
</script>

<template>
  <main class="bg-background flex min-h-screen flex-col justify-start gap-1 p-6">
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
    <section aria-labelledby="progress-style-heading" class="mt-4 flex flex-col gap-2">
      <h2 id="progress-style-heading" class="text-lg font-medium">进度样式</h2>
      <ToggleGroup
        type="single"
        variant="outline"
        :disabled="isSavingSettings || !settings"
        :model-value="currentProgressStyle"
        @update:model-value="handleProgressStyleChange"
      >
        <ToggleGroupItem
          v-for="option in progressStyleOptions"
          :key="option.value"
          :value="option.value"
          :aria-label="option.label"
        >
          {{ option.label }}
        </ToggleGroupItem>
      </ToggleGroup>
      <p class="text-muted-foreground text-sm">
        背景渐变只覆盖已播放部分，并从左侧向当前位置逐渐增强。
      </p>
    </section>
    <section aria-labelledby="taskbar-occupancy-heading" class="mt-4 flex flex-col gap-2">
      <div class="flex items-center justify-between gap-3">
        <h2 id="taskbar-occupancy-heading" class="text-lg font-medium">任务栏占用区域</h2>
        <button
          type="button"
          class="text-muted-foreground hover:text-foreground text-sm underline-offset-4 hover:underline"
          @click="loadTaskbarOccupancy"
        >
          重新读取
        </button>
      </div>
      <p v-if="taskbarOccupancyError" role="alert" class="text-destructive text-sm">
        {{ taskbarOccupancyError }}
      </p>
      <template v-else-if="taskbarOccupancy">
        <p class="text-muted-foreground text-sm">
          来源：{{ taskbarOccupancySourceLabels[taskbarOccupancy.source] }}；检测到
          {{ taskbarOccupancy.regions.length }} 个区域
        </p>
        <p
          v-if="taskbarOccupancy.fallbackReason"
          class="text-muted-foreground rounded-md border px-3 py-2 text-sm"
        >
          回退原因：{{ taskbarOccupancy.fallbackReason }}
        </p>
        <p v-if="taskbarOccupancy.regions.length === 0" class="text-muted-foreground text-sm">
          当前没有读取到可用的任务栏控件矩形。
        </p>
        <ul v-else class="flex max-h-72 flex-col gap-1 overflow-auto text-sm">
          <li
            v-for="(region, index) in taskbarOccupancy.regions"
            :key="`${region.left}-${region.right}-${region.top}-${region.bottom}-${index}`"
            class="bg-muted grid grid-cols-[minmax(0,1fr)_auto] gap-x-3 rounded-md border px-3 py-2"
          >
            <span class="truncate" :title="region.name || region.className">
              {{ region.name || region.className || '未命名控件' }}
            </span>
            <code class="text-muted-foreground">
              {{ region.left }},{{ region.top }} → {{ region.right }},{{ region.bottom }}
            </code>
            <code class="text-muted-foreground col-span-2 truncate" :title="region.className">
              {{ region.className || '无类名' }} · {{ region.width }}×{{ region.height }}
            </code>
          </li>
        </ul>
      </template>
      <p v-else class="text-muted-foreground text-sm">正在读取任务栏控件…</p>
    </section>
    <section aria-labelledby="media-snapshot-heading" class="mt-4 flex flex-col gap-2">
      <h2 id="media-snapshot-heading" class="text-lg font-medium">MediaSnapshot</h2>
      <p v-if="mediaSnapshotError" role="alert" class="text-destructive text-sm">
        {{ mediaSnapshotError }}
      </p>
      <div class="bg-muted flex flex-col gap-3 rounded-md border p-3">
        <div>
          <h3 class="font-medium">Seek 验证</h3>
          <p class="text-muted-foreground text-sm">拖动时只预览，释放后向播放器提交一次。</p>
        </div>
        <Slider
          :model-value="seekValue"
          :min="seekMinimum"
          :max="seekMaximum"
          :step="1000"
          :disabled="!canSeek || isSeekPending"
          aria-label="跳转播放位置"
          @update:model-value="handleSeekPreview"
          @value-commit="commitSeek"
        />
        <div class="text-muted-foreground flex justify-between text-xs tabular-nums">
          <span>{{ formatDuration(seekMinimum) }}</span>
          <span>{{ formatDuration(seekValue[0] ?? 0) }}</span>
          <span>{{ formatDuration(seekMaximum) }}</span>
        </div>
        <p
          v-if="seekFeedback"
          :class="
            seekFeedback.startsWith('跳转失败') ? 'text-destructive' : 'text-muted-foreground'
          "
          class="text-sm"
        >
          {{ seekFeedback }}
        </p>
        <p v-else-if="!canSeek" class="text-muted-foreground text-sm">
          当前会话未提供可用的 seek 能力或有效时间轴。
        </p>
      </div>
      <pre
        v-if="!mediaSnapshotError"
        class="bg-muted max-h-72 overflow-auto rounded-md border p-3 text-xs whitespace-pre-wrap"
        >{{ mediaSnapshotJson }}</pre>
      <h3 class="mt-2 font-medium">检测到的媒体会话</h3>
      <p v-if="mediaSessionIdentities.length === 0" class="text-muted-foreground text-sm">
        当前没有媒体会话
      </p>
      <ul v-else class="flex flex-col gap-1 text-sm">
        <li
          v-for="(identity, index) in mediaSessionIdentities"
          :key="`${identity.sourceAppId}-${index}`"
          class="bg-muted rounded-md border px-3 py-2"
        >
          <span class="font-medium">{{ playerKindLabels[identity.playerKind] }}</span>
          <code class="text-muted-foreground ml-2 break-all">{{ identity.sourceAppId }}</code>
        </li>
      </ul>
      <h3 class="mt-2 font-medium">播放器活动记录</h3>
      <p v-if="mediaSessionActivities.length === 0" class="text-muted-foreground text-sm">
        暂无活动记录
      </p>
      <dl v-else class="flex flex-col gap-2 text-sm">
        <div
          v-for="activity in mediaSessionActivities"
          :key="activity.sessionKey"
          class="bg-muted grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 rounded-md border px-3 py-2"
        >
          <dt class="text-muted-foreground">播放器</dt>
          <dd>{{ playerKindLabels[activity.playerKind] }} · {{ activity.sourceAppId }}</dd>
          <dt class="text-muted-foreground">曲目</dt>
          <dd>{{ activity.title || '未知标题' }} · {{ activity.artist || '未知歌手' }}</dd>
          <dt class="text-muted-foreground">播放</dt>
          <dd>
            {{ activity.isPlaying ? '播放中' : activity.isPaused ? '已暂停' : '其他状态' }}
          </dd>
          <dt class="text-muted-foreground">活动序号</dt>
          <dd>{{ activity.activitySequence ?? '尚无' }}</dd>
          <dt class="text-muted-foreground">活动原因</dt>
          <dd>
            {{
              activity.lastActivityReason
                ? activityReasonLabels[activity.lastActivityReason]
                : '尚无'
            }}
          </dd>
          <dt class="text-muted-foreground">活动时间</dt>
          <dd>
            {{
              activity.lastActivityAtUnixMs
                ? new Date(activity.lastActivityAtUnixMs).toLocaleString()
                : '尚无'
            }}
          </dd>
        </div>
      </dl>
    </section>
  </main>
</template>
