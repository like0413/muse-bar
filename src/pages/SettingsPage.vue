<script setup lang="ts">
import {
  ActivityIcon,
  AlertCircleIcon,
  FolderOpenIcon,
  InfoIcon,
  Music2Icon,
  PaletteIcon,
  PanelTopIcon,
  RefreshCwIcon,
  Settings2Icon,
} from '@lucide/vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldTitle,
} from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
} from '@/components/ui/sidebar'
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { waitForColorModeReady } from '@/lib/color-mode'
import {
  getCurrentMediaSnapshot,
  getMediaSessionActivities,
  getMediaSessionIdentities,
  listenToCurrentMediaSnapshotChanges,
  listenToCurrentTimelineChanges,
  listenToMediaSessionActivityChanges,
  listenToMediaSessionIdentityChanges,
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
  readControlPosition,
  readCustomProgressColor,
  readLaunchOnStartup,
  readLyricsAlignment,
  readLyricsEnabled,
  readManualOffset,
  readMaximumWidth,
  readProgressColorSource,
  readProgressStyle,
  readShowControls,
  readShowProgress,
  readTargetMonitor,
  readTaskbarPosition,
  readTitleScrollEnabled,
  readTitleScrollMode,
  readTitleScrollSpeed,
  readWindowMode,
  updateSettings,
  type ColorMode,
  type ControlPosition,
  type LyricsAlignment,
  type ProgressColorSource,
  type ProgressStyle,
  type SettingsPayload,
  type TaskbarPosition,
  type TitleScrollMode,
} from '@/lib/settings-api'
import { showReadySettingsWindow } from '@/lib/settings-window'
import {
  getTaskbarDpi,
  getTaskbarIdentity,
  getTaskbarOccupiedRegions,
  getWindowsVersion,
  openLogDirectory,
  type TaskbarDpi,
  type TaskbarIdentity,
  type TaskbarOccupancy,
  type WindowsVersion,
} from '@/lib/taskbar-diagnostics-api'
import { getTaskbarMonitors, type TaskbarMonitor } from '@/lib/taskbar-monitor-api'
import { readCurrentWindowLabel } from '@/lib/window-label'
import type { RuntimeInfo } from '@/types/runtime-info'

type SettingsSection = 'taskbar' | 'appearance' | 'media' | 'general' | 'diagnostics'

const MAXIMUM_WIDTH_SLIDER_MINIMUM = 200
const MAXIMUM_WIDTH_SLIDER_MAXIMUM = 520
const WIDTH_SLIDER_STEP = 4
const TITLE_SCROLL_SPEED_MINIMUM = 10
const TITLE_SCROLL_SPEED_MAXIMUM = 100
const TITLE_SCROLL_SPEED_STEP = 5

const activeSection = ref<SettingsSection>('taskbar')
const windowLabel = readCurrentWindowLabel()
const runtimeInfo = ref<RuntimeInfo>()
const runtimeError = ref<string>()
const windowsVersion = ref<WindowsVersion>()
const settings = ref<SettingsPayload>()
const settingsError = ref<string>()
const isSavingSettings = ref(false)
const taskbarIdentity = ref<TaskbarIdentity>()
const taskbarDpi = ref<TaskbarDpi>()
const taskbarOccupancy = ref<TaskbarOccupancy>()
const taskbarDiagnosticError = ref<string>()
const taskbarMonitorError = ref<string>()
const taskbarMonitors = ref<TaskbarMonitor[]>([])
const mediaSnapshot = ref<MediaSnapshot | null>(null)
const mediaSnapshotError = ref<string>()
const mediaSessionIdentities = ref<MediaSessionIdentity[]>([])
const mediaSessionActivities = ref<MediaSessionActivity[]>([])
const logDirectoryError = ref<string>()
const maxWidthDraft = ref<number[]>([])
const manualOffsetDraft = ref('0')
const customProgressColorDraft = ref('#0078D4')
const titleScrollSpeedDraft = ref<number[]>([])
let stopMediaSnapshotListener: UnlistenFn | undefined
let stopTimelineListener: UnlistenFn | undefined
let stopMediaSessionIdentityListener: UnlistenFn | undefined
let stopMediaSessionActivityListener: UnlistenFn | undefined
let hasUnmounted = false

const currentPosition = computed(() => readTaskbarPosition(settings.value))
const currentTargetMonitor = computed(() => readTargetMonitor(settings.value))
const targetMonitorSelection = computed(() => {
  const selected = currentTargetMonitor.value
  return taskbarMonitors.value.some((monitor) => monitor.id === selected) ? selected : 'primary'
})
const currentColorMode = computed(() => readColorMode(settings.value))
const showControls = computed(() => readShowControls(settings.value))
const currentControlPosition = computed(() => readControlPosition(settings.value))
const showProgress = computed(() => readShowProgress(settings.value))
const currentProgressStyle = computed(() => readProgressStyle(settings.value))
const currentProgressColorSource = computed(() => readProgressColorSource(settings.value))
const titleScrollEnabled = computed(() => readTitleScrollEnabled(settings.value))
const currentTitleScrollMode = computed(() => readTitleScrollMode(settings.value))
const currentWindowMode = computed(() => readWindowMode(settings.value))
const launchOnStartup = computed(() => readLaunchOnStartup(settings.value))
const lyricsEnabled = computed(() => readLyricsEnabled(settings.value))
const currentLyricsAlignment = computed(() => readLyricsAlignment(settings.value))
const isCustomProgressColorValid = computed(() =>
  /^#[0-9a-f]{6}$/i.test(customProgressColorDraft.value.trim()),
)
const previewAccentColor = computed(() => {
  if (currentProgressColorSource.value === 'custom') return readCustomProgressColor(settings.value)
  if (currentProgressColorSource.value === 'system') {
    return mediaSnapshot.value?.systemAccentColor || '#0078D4'
  }
  return mediaSnapshot.value?.accentColor || '#0078D4'
})
const recentErrors = computed(() =>
  [
    runtimeError.value,
    settingsError.value,
    taskbarDiagnosticError.value,
    taskbarMonitorError.value,
    mediaSnapshotError.value,
  ]
    .filter((error): error is string => Boolean(error))
    .slice(-5),
)

const navigationItems: ReadonlyArray<{
  id: SettingsSection
  label: string
  icon: typeof PanelTopIcon
}> = [
  { id: 'taskbar', label: '任务栏', icon: PanelTopIcon },
  { id: 'appearance', label: '外观', icon: PaletteIcon },
  { id: 'media', label: '媒体', icon: Music2Icon },
  { id: 'general', label: '常规', icon: Settings2Icon },
  { id: 'diagnostics', label: '诊断与关于', icon: ActivityIcon },
]

const positionOptions: ReadonlyArray<{ value: TaskbarPosition; label: string }> = [
  { value: 'left', label: '靠左' },
  { value: 'right', label: '靠右' },
]
const lyricsAlignmentOptions: ReadonlyArray<{ value: LyricsAlignment; label: string }> = [
  { value: 'left', label: '左对齐' },
  { value: 'center', label: '居中' },
  { value: 'right', label: '右对齐' },
]
const colorModeOptions: ReadonlyArray<{ value: ColorMode; label: string }> = [
  { value: 'system', label: '跟随系统' },
  { value: 'dark', label: '深色' },
  { value: 'light', label: '浅色' },
]
const progressStyleOptions: ReadonlyArray<{ value: ProgressStyle; label: string }> = [
  { value: 'underline', label: '底部细线' },
  { value: 'background-gradient', label: '背景渐变' },
]
const controlPositionOptions: ReadonlyArray<{ value: ControlPosition; label: string }> = [
  { value: 'left', label: '左侧' },
  { value: 'right', label: '右侧' },
]
const progressColorSourceOptions: ReadonlyArray<{
  value: ProgressColorSource
  label: string
}> = [
  { value: 'artwork', label: '封面主色' },
  { value: 'system', label: '系统主题色' },
  { value: 'custom', label: '自定义' },
]
const customProgressColorPresets = [
  '#0078D4',
  '#00B7C3',
  '#107C10',
  '#6B69D6',
  '#C239B3',
  '#E74856',
  '#F7630C',
  '#FFB900',
] as const
const titleScrollModeOptions: ReadonlyArray<{ value: TitleScrollMode; label: string }> = [
  { value: 'continuous', label: '连续滚动' },
  { value: 'restart', label: '从头滚动' },
  { value: 'bounce', label: '来回滚动' },
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
  trackChanged: '切换歌曲',
  becameCurrent: '成为系统当前会话',
}
const taskbarOccupancySourceLabels = {
  uiAutomation: 'UI Automation',
  win32Fallback: 'Win32 子窗口回退',
} as const

/** 将 Unix 毫秒时间戳转换为本地时间。 */
function formatStartedAt(startedAtUnixMs: number): string {
  return new Date(startedAtUnixMs).toLocaleString()
}

/** 将布尔控制能力转换为诊断页统一使用的中文状态。 */
function formatCapability(supported: boolean): string {
  return supported ? '支持' : '不支持'
}

/** 保存一个局部设置补丁，并保留 Rust 返回的规范化完整设置。 */
async function saveSettingsPatch(patch: SettingsPayload): Promise<void> {
  if (!settings.value || isSavingSettings.value) return
  isSavingSettings.value = true
  settingsError.value = undefined
  try {
    settings.value = await updateSettings({ ...settings.value, ...patch })
  } catch (error) {
    settingsError.value = error instanceof Error ? error.message : String(error)
  } finally {
    isSavingSettings.value = false
  }
}

/** 接收单选组的未知值，只保存合法的任务栏位置。 */
function handlePositionChange(position: unknown): void {
  if ((position === 'left' || position === 'right') && position !== currentPosition.value)
    void saveSettingsPatch({ position })
}

/** 保存目标任务栏显示器的设备标识。 */
function handleTargetMonitorChange(targetMonitor: unknown): void {
  if (
    typeof targetMonitor === 'string' &&
    targetMonitor &&
    targetMonitor !== currentTargetMonitor.value
  ) {
    void saveSettingsPatch({ targetMonitor })
  }
}

/** 更新手动偏移输入草稿，提交前不触发原生窗口移动。 */
function handleManualOffsetDraftChange(value: string | number): void {
  manualOffsetDraft.value = String(value)
}

/** 保存 -200 到 200 之间的整数偏移；正值向右，负值向左。 */
function commitManualOffset(): void {
  const parsed = Number(manualOffsetDraft.value)
  if (!Number.isFinite(parsed)) return
  const manualOffset = Math.round(Math.min(200, Math.max(-200, parsed)))
  manualOffsetDraft.value = String(manualOffset)
  if (manualOffset !== readManualOffset(settings.value)) void saveSettingsPatch({ manualOffset })
}

/** 接收单选组的未知值，只保存合法的颜色模式。 */
function handleColorModeChange(colorMode: unknown): void {
  if (
    (colorMode === 'system' || colorMode === 'dark' || colorMode === 'light') &&
    colorMode !== currentColorMode.value
  )
    void saveSettingsPatch({ colorMode })
}

/** 接收单选组的未知值，只保存合法的进度样式。 */
function handleProgressStyleChange(progressStyle: unknown): void {
  if (
    (progressStyle === 'underline' || progressStyle === 'background-gradient') &&
    progressStyle !== currentProgressStyle.value
  )
    void saveSettingsPatch({ progressStyle })
}

/** 保存控制按钮显隐状态；隐藏时仍保留按钮位置偏好。 */
function handleShowControlsChange(show: boolean): void {
  if (show !== showControls.value) void saveSettingsPatch({ showControls: show })
}

/** 保存歌词模式开关；当前阶段使用十二个中文字作为歌词占位。 */
function handleLyricsEnabledChange(enabled: boolean): void {
  if (enabled !== lyricsEnabled.value) void saveSettingsPatch({ lyricsEnabled: enabled })
}

/** 接收单选组的未知值，只保存合法的歌词对齐方式。 */
function handleLyricsAlignmentChange(alignment: unknown): void {
  if (
    (alignment === 'left' || alignment === 'center' || alignment === 'right') &&
    alignment !== currentLyricsAlignment.value
  ) {
    void saveSettingsPatch({ lyricsAlignment: alignment })
  }
}

/** 接收单选组的未知值，只保存合法的控制按钮位置。 */
function handleControlPositionChange(position: unknown): void {
  if ((position === 'left' || position === 'right') && position !== currentControlPosition.value) {
    void saveSettingsPatch({ controlPosition: position })
  }
}

/** 保存进度视觉显隐状态；隐藏时仍保留下级样式与颜色。 */
function handleShowProgressChange(show: boolean): void {
  if (show !== showProgress.value) void saveSettingsPatch({ showProgress: show })
}

/** 接收单选组的未知值，只保存合法的进度颜色来源。 */
function handleProgressColorSourceChange(source: unknown): void {
  if (
    (source === 'artwork' || source === 'system' || source === 'custom') &&
    source !== currentProgressColorSource.value
  ) {
    void saveSettingsPatch({ progressColorSource: source })
  }
}

/** 选择预设颜色时立即保存，并同步手动输入框。 */
function handleCustomProgressColorPresetChange(color: unknown): void {
  if (typeof color !== 'string' || !/^#[0-9a-f]{6}$/i.test(color)) return
  customProgressColorDraft.value = color.toUpperCase()
  void saveSettingsPatch({ customProgressColor: customProgressColorDraft.value })
}

/** 更新自定义颜色输入草稿，只有提交时才写入配置。 */
function handleCustomProgressColorDraftChange(value: string | number): void {
  customProgressColorDraft.value = String(value)
}

/** 保存合法的六位十六进制颜色，并把文本统一为大写。 */
function commitCustomProgressColor(): void {
  const color = customProgressColorDraft.value.trim().toUpperCase()
  if (!/^#[0-9A-F]{6}$/.test(color)) return
  customProgressColorDraft.value = color
  if (color !== readCustomProgressColor(settings.value)) {
    void saveSettingsPatch({ customProgressColor: color })
  }
}

/** 保存标题滚动总开关；关闭后下级参数保留但不再应用。 */
function handleTitleScrollEnabledChange(enabled: boolean): void {
  if (enabled !== titleScrollEnabled.value) void saveSettingsPatch({ titleScrollEnabled: enabled })
}

/** 接收单选组的未知值，只保存合法的标题滚动方式。 */
function handleTitleScrollModeChange(mode: unknown): void {
  if (
    (mode === 'continuous' || mode === 'restart' || mode === 'bounce') &&
    mode !== currentTitleScrollMode.value
  )
    void saveSettingsPatch({ titleScrollMode: mode })
}

/** 更新标题滚动速度草稿，拖动期间只改变设置页显示。 */
function handleTitleScrollSpeedDraftChange(value: number[] | undefined): void {
  const speed = value?.[0]
  if (speed !== undefined) titleScrollSpeedDraft.value = [speed]
}

/** 提交标题滚动速度，避免拖动过程中连续写入配置文件。 */
function commitTitleScrollSpeed(value: number[]): void {
  handleTitleScrollSpeedDraftChange(value)
  const speed = titleScrollSpeedDraft.value[0]
  if (speed !== undefined) void saveSettingsPatch({ titleScrollSpeed: speed })
}

/** 保存开机启动开关，Rust 会同步 Windows 当前用户启动项。 */
function handleLaunchOnStartupChange(enabled: boolean): void {
  if (enabled !== launchOnStartup.value) void saveSettingsPatch({ launchOnStartup: enabled })
}

/** 更新普通模式最大宽度草稿。 */
function handleMaximumWidthDraftChange(value: number[] | undefined): void {
  const width = value?.[0]
  if (width === undefined) return
  maxWidthDraft.value = [width]
}

/** 提交普通模式最大宽度草稿。 */
function commitMaximumWidth(value: number[]): void {
  handleMaximumWidthDraftChange(value)
  const width = maxWidthDraft.value[0]
  if (width !== undefined) void saveSettingsPatch({ maxWidth: width })
}

/** 并行读取应用启动信息和 Windows 构建号，单项失败不遮住另一项。 */
async function loadRuntimeInfo(): Promise<void> {
  const [runtimeResult, windowsResult] = await Promise.allSettled([
    getRuntimeInfo(),
    getWindowsVersion(),
  ])
  if (runtimeResult.status === 'fulfilled') runtimeInfo.value = runtimeResult.value
  if (windowsResult.status === 'fulfilled') windowsVersion.value = windowsResult.value

  const errors: string[] = []
  if (runtimeResult.status === 'rejected') errors.push(String(runtimeResult.reason))
  if (windowsResult.status === 'rejected') errors.push(String(windowsResult.reason))
  runtimeError.value = errors.length > 0 ? errors.join('；') : undefined
}

/** 读取 Rust 持有的完整设置，供页面展示和局部更新。 */
async function loadSettings(): Promise<void> {
  try {
    settings.value = await getSettings()
    settingsError.value = undefined
  } catch (error) {
    settingsError.value = error instanceof Error ? error.message : String(error)
  }
}

/** 读取当前具有任务栏的显示器，供目标显示器下拉框使用。 */
async function loadTaskbarMonitors(): Promise<void> {
  try {
    taskbarMonitors.value = await getTaskbarMonitors()
    taskbarMonitorError.value = undefined
  } catch (error) {
    taskbarMonitorError.value = error instanceof Error ? error.message : String(error)
  }
}

/** 并行读取任务栏身份、DPI 和占用矩形，形成一次诊断刷新。 */
async function loadTaskbarDiagnostics(): Promise<void> {
  taskbarDiagnosticError.value = undefined
  try {
    const [identity, dpi, occupancy] = await Promise.all([
      getTaskbarIdentity(),
      getTaskbarDpi(),
      getTaskbarOccupiedRegions(),
    ])
    taskbarIdentity.value = identity
    taskbarDpi.value = dpi
    taskbarOccupancy.value = occupancy
  } catch (error) {
    taskbarDiagnosticError.value = error instanceof Error ? error.message : String(error)
  }
}

/** 打开 Rust 日志目录，并在失败时把原因留在诊断页。 */
async function handleOpenLogDirectory(): Promise<void> {
  logDirectoryError.value = undefined
  try {
    await openLogDirectory()
  } catch (error) {
    logDirectoryError.value = error instanceof Error ? error.message : String(error)
  }
}

/** 订阅统一媒体快照和轻量时间轴事件，并读取首次快照。 */
async function startMediaSnapshotListener(): Promise<void> {
  try {
    const stopSnapshot = await listenToCurrentMediaSnapshotChanges((snapshot) => {
      mediaSnapshot.value = snapshot
      mediaSnapshotError.value = undefined
    })
    if (hasUnmounted) return stopSnapshot()
    stopMediaSnapshotListener = stopSnapshot
    const stopTimeline = await listenToCurrentTimelineChanges((timeline) => {
      if (mediaSnapshot.value) mediaSnapshot.value = { ...mediaSnapshot.value, timeline }
    })
    if (hasUnmounted) return stopTimeline()
    stopTimelineListener = stopTimeline
    mediaSnapshot.value = await getCurrentMediaSnapshot()
  } catch (error) {
    mediaSnapshotError.value = error instanceof Error ? error.message : String(error)
  }
}

/** 订阅播放器会话列表，并读取页面打开前已存在的会话。 */
async function startMediaSessionIdentityListener(): Promise<void> {
  try {
    const stopListener = await listenToMediaSessionIdentityChanges((identities) => {
      mediaSessionIdentities.value = identities
    })
    if (hasUnmounted) return stopListener()
    stopMediaSessionIdentityListener = stopListener
    mediaSessionIdentities.value = await getMediaSessionIdentities()
  } catch (error) {
    mediaSnapshotError.value = error instanceof Error ? error.message : String(error)
  }
}

/** 订阅播放器活动记录，帮助解释当前媒体为什么被选中。 */
async function startMediaSessionActivityListener(): Promise<void> {
  try {
    const stopListener = await listenToMediaSessionActivityChanges((activities) => {
      mediaSessionActivities.value = activities
    })
    if (hasUnmounted) return stopListener()
    stopMediaSessionActivityListener = stopListener
    mediaSessionActivities.value = await getMediaSessionActivities()
  } catch (error) {
    mediaSnapshotError.value = error instanceof Error ? error.message : String(error)
  }
}

/** 等待浏览器至少获得一次绘制机会，同时用短超时避免隐藏窗口中动画帧被暂停。 */
function waitForInitialPaint(): Promise<void> {
  return new Promise((resolve) => {
    let resolved = false
    const finish = () => {
      if (resolved) return
      resolved = true
      resolve()
    }

    window.requestAnimationFrame(() => window.requestAnimationFrame(finish))
    window.setTimeout(finish, 50)
  })
}

/** 完成设置页首次数据读取、主题应用和 DOM 绘制后再显示原生窗口。 */
async function initializeSettingsPage(): Promise<void> {
  await Promise.all([
    loadRuntimeInfo(),
    loadSettings(),
    loadTaskbarMonitors(),
    loadTaskbarDiagnostics(),
    startMediaSnapshotListener(),
    startMediaSessionIdentityListener(),
    startMediaSessionActivityListener(),
    waitForColorModeReady(),
  ])
  if (hasUnmounted) return

  await nextTick()
  await waitForInitialPaint()
  if (hasUnmounted) return

  try {
    await showReadySettingsWindow()
  } catch (error) {
    console.error('设置页准备完成，但无法显示原生窗口：', error)
  }
}

watch(
  settings,
  (value) => {
    const maximumWidth = readMaximumWidth(value)
    const manualOffset = readManualOffset(value)
    const titleScrollSpeed = readTitleScrollSpeed(value)
    if (maximumWidth !== undefined) maxWidthDraft.value = [maximumWidth]
    if (manualOffset !== undefined) manualOffsetDraft.value = String(manualOffset)
    customProgressColorDraft.value = readCustomProgressColor(value)
    titleScrollSpeedDraft.value = [titleScrollSpeed]
  },
  { immediate: true },
)

onMounted(() => void initializeSettingsPage())

onBeforeUnmount(() => {
  hasUnmounted = true
  stopMediaSnapshotListener?.()
  stopTimelineListener?.()
  stopMediaSessionIdentityListener?.()
  stopMediaSessionActivityListener?.()
})
</script>

<template>
  <SidebarProvider class="min-h-screen">
    <Sidebar collapsible="none" class="border-r">
      <SidebarHeader class="border-b p-4">
        <div class="flex items-center gap-3">
          <div
            class="bg-primary text-primary-foreground flex size-9 items-center justify-center rounded-lg"
          >
            <Music2Icon class="size-5" />
          </div>
          <div>
            <p class="font-semibold">Muse Bar</p>
            <p class="text-muted-foreground text-xs">设置</p>
          </div>
        </div>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>设置</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem v-for="item in navigationItems" :key="item.id">
                <SidebarMenuButton
                  :is-active="activeSection === item.id"
                  @click="activeSection = item.id"
                >
                  <component :is="item.icon" /><span>{{ item.label }}</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>

    <SidebarInset class="min-w-0">
      <header
        class="bg-background/95 sticky top-0 z-10 flex h-16 items-center border-b px-6 backdrop-blur"
      >
        <div>
          <h1 class="text-lg font-semibold">
            {{ navigationItems.find((item) => item.id === activeSection)?.label }}
          </h1>
          <p class="text-muted-foreground text-xs">修改会立即同步到任务栏 Bar</p>
        </div>
      </header>

      <main class="mx-auto flex w-full max-w-4xl flex-1 flex-col gap-6 p-6">
        <Alert v-if="settingsError" variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>设置保存失败</AlertTitle>
          <AlertDescription>{{ settingsError }}</AlertDescription>
        </Alert>
        <Alert v-if="taskbarMonitorError && activeSection === 'taskbar'" variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>显示器列表读取失败</AlertTitle>
          <AlertDescription>{{ taskbarMonitorError }}</AlertDescription>
        </Alert>

        <template v-if="activeSection === 'taskbar'">
          <Card>
            <CardHeader>
              <CardTitle>位置与尺寸</CardTitle>
              <CardDescription>控制 Bar 所在的任务栏、位置和内容宽度范围。</CardDescription>
            </CardHeader>
            <CardContent>
              <FieldGroup>
                <Field>
                  <FieldLabel>目标显示器</FieldLabel>
                  <Select
                    :model-value="targetMonitorSelection"
                    :disabled="isSavingSettings || !settings || taskbarMonitors.length === 0"
                    @update:model-value="handleTargetMonitorChange"
                  >
                    <SelectTrigger class="w-full">
                      <SelectValue placeholder="选择具有任务栏的显示器" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem
                        v-for="monitor in taskbarMonitors"
                        :key="monitor.id"
                        :value="monitor.id"
                      >
                        {{ monitor.label }}
                      </SelectItem>
                    </SelectContent>
                  </Select>
                  <FieldDescription>只列出当前具有 Windows 任务栏的显示器。</FieldDescription>
                </Field>
                <Field>
                  <FieldLabel>任务栏位置</FieldLabel>
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
                      >{{ option.label }}</ToggleGroupItem
                    >
                  </ToggleGroup>
                  <FieldDescription>Bar 会贴近所选一侧的任务栏组件。</FieldDescription>
                </Field>
                <Field>
                  <div class="flex items-center justify-between gap-4">
                    <FieldLabel>普通模式最大宽度</FieldLabel>
                    <Badge variant="outline"
                      >{{ maxWidthDraft[0] ?? '读取中'
                      }}<template v-if="maxWidthDraft[0] !== undefined"> px</template></Badge
                    >
                  </div>
                  <Slider
                    aria-label="Bar 普通模式最大宽度"
                    :model-value="maxWidthDraft"
                    :min="MAXIMUM_WIDTH_SLIDER_MINIMUM"
                    :max="MAXIMUM_WIDTH_SLIDER_MAXIMUM"
                    :step="WIDTH_SLIDER_STEP"
                    :disabled="isSavingSettings || !settings"
                    @update:model-value="handleMaximumWidthDraftChange"
                    @value-commit="commitMaximumWidth"
                  />
                  <FieldDescription>
                    普通模式按内容自然收缩；歌词模式改为占满对应任务栏空白区域。
                  </FieldDescription>
                </Field>
                <Field>
                  <div class="flex items-center justify-between gap-4">
                    <FieldContent>
                      <FieldTitle>手动偏移</FieldTitle>
                      <FieldDescription>正值向右移动，负值向左移动。</FieldDescription>
                    </FieldContent>
                    <Badge variant="outline">{{ manualOffsetDraft }} px</Badge>
                  </div>
                  <Input
                    type="number"
                    inputmode="numeric"
                    aria-label="Bar 手动偏移"
                    :model-value="manualOffsetDraft"
                    :min="-200"
                    :max="200"
                    :disabled="isSavingSettings || !settings"
                    @update:model-value="handleManualOffsetDraftChange"
                    @blur="commitManualOffset"
                    @keydown.enter="commitManualOffset"
                  />
                </Field>
              </FieldGroup>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>歌词模式</CardTitle>
              <CardDescription>
                当前使用十二字占位歌词验证宽度与悬停动画，后续再接入真实歌词。
              </CardDescription>
            </CardHeader>
            <CardContent>
              <FieldGroup>
                <Field orientation="horizontal">
                  <FieldContent>
                    <FieldTitle>显示歌词</FieldTitle>
                    <FieldDescription>
                      默认显示“这是一句十二字占位歌词呀”，悬停时切换为媒体信息。
                    </FieldDescription>
                  </FieldContent>
                  <Switch
                    :model-value="lyricsEnabled"
                    :disabled="isSavingSettings || !settings"
                    aria-label="显示歌词"
                    @update:model-value="handleLyricsEnabledChange"
                  />
                </Field>
                <Field :data-disabled="!lyricsEnabled">
                  <FieldLabel>歌词对齐方式</FieldLabel>
                  <ToggleGroup
                    type="single"
                    variant="outline"
                    :model-value="currentLyricsAlignment"
                    :disabled="isSavingSettings || !settings || !lyricsEnabled"
                    @update:model-value="handleLyricsAlignmentChange"
                  >
                    <ToggleGroupItem
                      v-for="option in lyricsAlignmentOptions"
                      :key="option.value"
                      :value="option.value"
                    >
                      {{ option.label }}
                    </ToggleGroupItem>
                  </ToggleGroup>
                  <FieldDescription>控制歌词在封面右侧区域中的水平位置。</FieldDescription>
                </Field>
              </FieldGroup>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>宿主模式</CardTitle>
              <CardDescription>当前版本使用已经验证通过的 Child 真嵌入方案。</CardDescription>
            </CardHeader>
            <CardContent>
              <Field orientation="horizontal">
                <FieldContent>
                  <FieldTitle>Owner 兼容模式</FieldTitle>
                  <FieldDescription
                    >Owner 留待以后实现；当前实际模式为
                    {{
                      currentWindowMode === 'auto' ? 'Child' : currentWindowMode
                    }}。</FieldDescription
                  >
                </FieldContent>
                <Switch :model-value="false" disabled aria-label="Owner 兼容模式暂未启用" />
              </Field>
            </CardContent>
          </Card>
        </template>

        <template v-else-if="activeSection === 'appearance'">
          <Card>
            <CardHeader>
              <CardTitle>颜色模式</CardTitle>
              <CardDescription>跟随 Windows，或固定 Muse Bar 的明暗主题。</CardDescription>
            </CardHeader>
            <CardContent>
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
                  >{{ option.label }}</ToggleGroupItem
                >
              </ToggleGroup>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>控制按钮</CardTitle>
              <CardDescription>控制播放按钮是否出现，以及它们位于 Bar 的哪一侧。</CardDescription>
            </CardHeader>
            <CardContent>
              <FieldGroup>
                <Field orientation="horizontal">
                  <FieldContent>
                    <FieldTitle>显示控制按钮</FieldTitle>
                    <FieldDescription>包括上一曲、播放/暂停和下一曲。</FieldDescription>
                  </FieldContent>
                  <Switch
                    :model-value="showControls"
                    :disabled="isSavingSettings || !settings"
                    aria-label="显示控制按钮"
                    @update:model-value="handleShowControlsChange"
                  />
                </Field>
                <Field :data-disabled="!showControls">
                  <FieldLabel>按钮位置</FieldLabel>
                  <ToggleGroup
                    type="single"
                    variant="outline"
                    :model-value="currentControlPosition"
                    :disabled="isSavingSettings || !settings || !showControls"
                    @update:model-value="handleControlPositionChange"
                  >
                    <ToggleGroupItem
                      v-for="option in controlPositionOptions"
                      :key="option.value"
                      :value="option.value"
                    >
                      {{ option.label }}
                    </ToggleGroupItem>
                  </ToggleGroup>
                </Field>
              </FieldGroup>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>播放进度</CardTitle>
              <CardDescription>控制进度视觉的显隐、样式和颜色来源。</CardDescription>
            </CardHeader>
            <CardContent class="flex flex-col gap-6">
              <FieldGroup>
                <Field orientation="horizontal">
                  <FieldContent>
                    <FieldTitle>显示播放进度</FieldTitle>
                    <FieldDescription>关闭后不会显示底线或背景渐变。</FieldDescription>
                  </FieldContent>
                  <Switch
                    :model-value="showProgress"
                    :disabled="isSavingSettings || !settings"
                    aria-label="显示播放进度"
                    @update:model-value="handleShowProgressChange"
                  />
                </Field>
                <Field :data-disabled="!showProgress">
                  <FieldLabel>进度样式</FieldLabel>
                  <ToggleGroup
                    type="single"
                    variant="outline"
                    :disabled="isSavingSettings || !settings || !showProgress"
                    :model-value="currentProgressStyle"
                    @update:model-value="handleProgressStyleChange"
                  >
                    <ToggleGroupItem
                      v-for="option in progressStyleOptions"
                      :key="option.value"
                      :value="option.value"
                    >
                      {{ option.label }}
                    </ToggleGroupItem>
                  </ToggleGroup>
                </Field>
                <Field :data-disabled="!showProgress">
                  <FieldLabel>进度颜色</FieldLabel>
                  <ToggleGroup
                    type="single"
                    variant="outline"
                    :model-value="currentProgressColorSource"
                    :disabled="isSavingSettings || !settings || !showProgress"
                    @update:model-value="handleProgressColorSourceChange"
                  >
                    <ToggleGroupItem
                      v-for="option in progressColorSourceOptions"
                      :key="option.value"
                      :value="option.value"
                    >
                      {{ option.label }}
                    </ToggleGroupItem>
                  </ToggleGroup>
                </Field>
                <Field
                  v-if="currentProgressColorSource === 'custom'"
                  :data-disabled="!showProgress"
                  :data-invalid="!isCustomProgressColorValid"
                >
                  <FieldLabel>自定义颜色</FieldLabel>
                  <ToggleGroup
                    type="single"
                    variant="outline"
                    class="flex-wrap justify-start"
                    :model-value="readCustomProgressColor(settings)"
                    :disabled="isSavingSettings || !settings || !showProgress"
                    @update:model-value="handleCustomProgressColorPresetChange"
                  >
                    <ToggleGroupItem
                      v-for="color in customProgressColorPresets"
                      :key="color"
                      :value="color"
                      :aria-label="`使用颜色 ${color}`"
                      class="size-9 p-1"
                    >
                      <span
                        class="size-full rounded-sm border border-black/10"
                        :style="{ backgroundColor: color }"
                      />
                    </ToggleGroupItem>
                  </ToggleGroup>
                  <Input
                    :model-value="customProgressColorDraft"
                    placeholder="#0078D4"
                    maxlength="7"
                    spellcheck="false"
                    :aria-invalid="!isCustomProgressColorValid"
                    :disabled="isSavingSettings || !settings || !showProgress"
                    @update:model-value="handleCustomProgressColorDraftChange"
                    @blur="commitCustomProgressColor"
                    @keydown.enter="commitCustomProgressColor"
                  />
                  <FieldDescription> 输入“#”加六位十六进制颜色，例如 #FF5A5F。 </FieldDescription>
                </Field>
              </FieldGroup>
              <div class="bg-muted flex min-h-36 items-center justify-center rounded-xl border p-6">
                <div
                  class="bg-card text-card-foreground relative flex h-14 w-full max-w-md items-center gap-3 overflow-hidden rounded-xl border px-3 shadow-sm"
                >
                  <div
                    v-if="showProgress && currentProgressStyle === 'background-gradient'"
                    class="pointer-events-none absolute inset-y-0 left-0 w-3/5"
                    :style="{
                      background: `linear-gradient(90deg, transparent, color-mix(in srgb, ${previewAccentColor} 42%, transparent))`,
                    }"
                  />
                  <div
                    v-if="showControls && currentControlPosition === 'left'"
                    class="relative shrink-0 text-sm"
                    aria-hidden="true"
                  >
                    ◀　Ⅱ　▶
                  </div>
                  <Avatar class="relative size-10 rounded-md">
                    <AvatarImage
                      v-if="mediaSnapshot?.artworkDataUrl"
                      :src="mediaSnapshot.artworkDataUrl"
                    />
                    <AvatarFallback class="rounded-md">
                      <Music2Icon class="size-4" />
                    </AvatarFallback>
                  </Avatar>
                  <div class="relative min-w-0 flex-1">
                    <p class="truncate text-sm font-medium">
                      {{ mediaSnapshot?.title || 'Muse Bar 预览' }}
                    </p>
                    <p class="text-muted-foreground truncate text-xs">
                      {{ mediaSnapshot?.artist || '当前歌曲歌手' }}
                    </p>
                  </div>
                  <div
                    v-if="showControls && currentControlPosition === 'right'"
                    class="relative shrink-0 text-sm"
                    aria-hidden="true"
                  >
                    ◀　Ⅱ　▶
                  </div>
                  <div
                    v-if="showProgress && currentProgressStyle === 'underline'"
                    class="absolute bottom-0 left-0 h-0.5 w-3/5"
                    :style="{ backgroundColor: previewAccentColor }"
                  />
                </div>
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>滚动文本</CardTitle>
              <CardDescription>歌曲名超过可用宽度时自动滚动，短标题始终保持静止。</CardDescription>
            </CardHeader>
            <CardContent>
              <FieldGroup>
                <Field orientation="horizontal">
                  <FieldContent>
                    <FieldTitle>滚动长标题</FieldTitle>
                    <FieldDescription>只影响歌曲名，不滚动歌手信息。</FieldDescription>
                  </FieldContent>
                  <Switch
                    :model-value="titleScrollEnabled"
                    :disabled="isSavingSettings || !settings"
                    aria-label="滚动长标题"
                    @update:model-value="handleTitleScrollEnabledChange"
                  />
                </Field>
                <Field :data-disabled="!titleScrollEnabled">
                  <div class="flex items-center justify-between gap-4">
                    <FieldLabel>滚动速度</FieldLabel>
                    <Badge variant="outline"
                      >{{ titleScrollSpeedDraft[0] ?? '读取中'
                      }}<template v-if="titleScrollSpeedDraft[0] !== undefined">
                        px/s</template
                      ></Badge
                    >
                  </div>
                  <Slider
                    aria-label="标题滚动速度"
                    :model-value="titleScrollSpeedDraft"
                    :min="TITLE_SCROLL_SPEED_MINIMUM"
                    :max="TITLE_SCROLL_SPEED_MAXIMUM"
                    :step="TITLE_SCROLL_SPEED_STEP"
                    :disabled="isSavingSettings || !settings || !titleScrollEnabled"
                    @update:model-value="handleTitleScrollSpeedDraftChange"
                    @value-commit="commitTitleScrollSpeed"
                  />
                  <FieldDescription>数值越大，标题每秒移动的距离越远。</FieldDescription>
                </Field>
                <Field :data-disabled="!titleScrollEnabled">
                  <FieldLabel>滚动方式</FieldLabel>
                  <ToggleGroup
                    type="single"
                    variant="outline"
                    :model-value="currentTitleScrollMode"
                    :disabled="isSavingSettings || !settings || !titleScrollEnabled"
                    @update:model-value="handleTitleScrollModeChange"
                  >
                    <ToggleGroupItem
                      v-for="option in titleScrollModeOptions"
                      :key="option.value"
                      :value="option.value"
                      >{{ option.label }}</ToggleGroupItem
                    >
                  </ToggleGroup>
                  <FieldDescription>
                    连续滚动会首尾衔接；从头滚动会在末尾重置；来回滚动会在两端之间往返。
                  </FieldDescription>
                </Field>
              </FieldGroup>
            </CardContent>
          </Card>
        </template>

        <template v-else-if="activeSection === 'media'">
          <Alert v-if="mediaSnapshotError" variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>媒体状态读取失败</AlertTitle>
            <AlertDescription>{{ mediaSnapshotError }}</AlertDescription>
          </Alert>
          <Card>
            <CardHeader>
              <CardTitle>当前展示媒体</CardTitle>
              <CardDescription>Bar 当前选择的系统媒体会话。</CardDescription>
            </CardHeader>
            <CardContent>
              <div v-if="mediaSnapshot" class="flex items-center gap-4">
                <Avatar class="size-16 rounded-lg">
                  <AvatarImage
                    v-if="mediaSnapshot.artworkDataUrl"
                    :src="mediaSnapshot.artworkDataUrl"
                  />
                  <AvatarFallback class="rounded-lg">
                    <Music2Icon />
                  </AvatarFallback>
                </Avatar>
                <div class="min-w-0 flex-1">
                  <p class="truncate font-semibold">{{ mediaSnapshot.title || '未知歌曲' }}</p>
                  <p class="text-muted-foreground truncate text-sm">
                    {{ mediaSnapshot.artist || '未知歌手' }}
                  </p>
                  <div class="mt-2 flex flex-wrap gap-2">
                    <Badge>{{ playerKindLabels[mediaSnapshot.playerKind] }}</Badge>
                    <Badge variant="outline">{{ mediaSnapshot.playbackStatus }}</Badge>
                    <Badge variant="secondary">当前选择项</Badge>
                  </div>
                </div>
              </div>
              <p v-else class="text-muted-foreground text-sm">当前没有可展示的系统媒体会话。</p>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>控制能力</CardTitle>
              <CardDescription>是否可用由播放器通过 Windows SMTC 声明。</CardDescription>
            </CardHeader>
            <CardContent>
              <dl v-if="mediaSnapshot" class="grid grid-cols-2 gap-3 sm:grid-cols-5">
                <div
                  v-for="capability in [
                    ['播放', mediaSnapshot.capabilities.canPlay],
                    ['暂停', mediaSnapshot.capabilities.canPause],
                    ['上一曲', mediaSnapshot.capabilities.canPrevious],
                    ['下一曲', mediaSnapshot.capabilities.canNext],
                    ['跳转', mediaSnapshot.capabilities.canSeek],
                  ]"
                  :key="String(capability[0])"
                  class="bg-muted rounded-lg border p-3 text-center"
                >
                  <dt class="text-sm font-medium">{{ capability[0] }}</dt>
                  <dd class="text-muted-foreground mt-1 text-xs">
                    {{ formatCapability(Boolean(capability[1])) }}
                  </dd>
                </div>
              </dl>
              <p v-else class="text-muted-foreground text-sm">选择媒体会话后显示控制能力。</p>
              <p class="text-muted-foreground mt-4 text-sm">
                不支持的能力会使 Bar 上对应按钮禁用，这通常表示播放器没有向 Windows 暴露该操作。
              </p>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>检测到的会话</CardTitle>
              <CardDescription
                >共 {{ mediaSessionIdentities.length }} 个系统媒体会话。</CardDescription
              >
            </CardHeader>
            <CardContent class="flex flex-col gap-2">
              <p v-if="mediaSessionIdentities.length === 0" class="text-muted-foreground text-sm">
                暂无媒体会话。
              </p>
              <template v-else>
                <div
                  v-for="identity in mediaSessionIdentities"
                  :key="identity.sessionKey"
                  class="flex items-center justify-between gap-3 rounded-lg border p-3"
                >
                  <div class="min-w-0">
                    <p class="font-medium">{{ playerKindLabels[identity.playerKind] }}</p>
                    <code class="text-muted-foreground block truncate text-xs">{{
                      identity.sourceAppId
                    }}</code>
                  </div>
                  <Badge v-if="identity.sessionKey === mediaSnapshot?.sessionKey">正在显示</Badge>
                </div>
              </template>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>最近活动</CardTitle>
              <CardDescription>用于解释多个播放器同时存在时的选择顺序。</CardDescription>
            </CardHeader>
            <CardContent class="flex flex-col gap-2">
              <p v-if="mediaSessionActivities.length === 0" class="text-muted-foreground text-sm">
                暂无有效活动记录。
              </p>
              <template v-else>
                <div
                  v-for="activity in mediaSessionActivities"
                  :key="activity.sessionKey"
                  class="rounded-lg border p-3"
                >
                  <div class="flex items-center justify-between gap-3">
                    <p class="truncate font-medium">
                      {{ activity.title || playerKindLabels[activity.playerKind] }}
                    </p>
                    <Badge variant="outline">{{
                      activity.lastActivityReason
                        ? activityReasonLabels[activity.lastActivityReason]
                        : '尚未活动'
                    }}</Badge>
                  </div>
                  <p class="text-muted-foreground mt-1 truncate text-xs">
                    {{ activity.artist || activity.sourceAppId }}
                  </p>
                </div>
              </template>
            </CardContent>
          </Card>
        </template>

        <template v-else-if="activeSection === 'general'">
          <Card>
            <CardHeader>
              <CardTitle>应用行为</CardTitle>
              <CardDescription>控制 Muse Bar 的 Windows 启动和后台运行行为。</CardDescription>
            </CardHeader>
            <CardContent>
              <FieldGroup>
                <Field orientation="horizontal">
                  <FieldContent>
                    <FieldTitle>开机启动</FieldTitle>
                    <FieldDescription
                      >使用 Windows 当前用户启动项，不需要管理员权限。</FieldDescription
                    >
                  </FieldContent>
                  <Switch
                    :model-value="launchOnStartup"
                    :disabled="isSavingSettings || !settings"
                    aria-label="开机启动"
                    @update:model-value="handleLaunchOnStartupChange"
                  />
                </Field>
                <Field orientation="horizontal">
                  <FieldContent>
                    <FieldTitle>单实例与托盘</FieldTitle>
                    <FieldDescription>
                      重复启动会唤醒本窗口；关闭设置页后应用继续留在托盘。
                    </FieldDescription>
                  </FieldContent>
                  <Badge variant="secondary">已启用</Badge>
                </Field>
              </FieldGroup>
            </CardContent>
          </Card>
        </template>

        <template v-else>
          <div class="flex justify-end gap-2">
            <Button variant="outline" size="sm" @click="loadTaskbarDiagnostics">
              <RefreshCwIcon data-icon="inline-start" />刷新诊断
            </Button>
            <Button size="sm" @click="handleOpenLogDirectory">
              <FolderOpenIcon data-icon="inline-start" />打开日志目录
            </Button>
          </div>
          <Alert v-if="logDirectoryError" variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>无法打开日志目录</AlertTitle>
            <AlertDescription>{{ logDirectoryError }}</AlertDescription>
          </Alert>
          <Card>
            <CardHeader>
              <CardTitle>运行环境</CardTitle>
              <CardDescription>Muse Bar 当前进程与窗口信息。</CardDescription>
            </CardHeader>
            <CardContent>
              <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-3 text-sm">
                <dt class="text-muted-foreground">Windows</dt>
                <dd>
                  {{
                    windowsVersion
                      ? `${windowsVersion.productName} · ${windowsVersion.version}（Build ${windowsVersion.build}）`
                      : '读取中'
                  }}
                </dd>
                <dt class="text-muted-foreground">应用版本</dt>
                <dd>{{ runtimeInfo?.applicationVersion || '读取中' }}</dd>
                <dt class="text-muted-foreground">启动时间</dt>
                <dd>{{ runtimeInfo ? formatStartedAt(runtimeInfo.startedAtUnixMs) : '读取中' }}</dd>
                <dt class="text-muted-foreground">窗口标签</dt>
                <dd>
                  <code>{{ windowLabel }}</code>
                </dd>
                <dt class="text-muted-foreground">实际宿主</dt>
                <dd>
                  <Badge>Child</Badge>
                </dd>
              </dl>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>任务栏状态</CardTitle>
              <CardDescription>主任务栏身份、DPI 和原生控件测量结果。</CardDescription>
            </CardHeader>
            <CardContent class="flex flex-col gap-4">
              <Alert v-if="taskbarDiagnosticError" variant="destructive">
                <AlertCircleIcon />
                <AlertTitle>任务栏诊断失败</AlertTitle>
                <AlertDescription>{{ taskbarDiagnosticError }}</AlertDescription>
              </Alert>
              <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-3 text-sm">
                <dt class="text-muted-foreground">任务栏句柄</dt>
                <dd>
                  <code>{{
                    taskbarIdentity
                      ? `0x${taskbarIdentity.hwnd.toString(16).toUpperCase()}`
                      : '读取中'
                  }}</code>
                </dd>
                <dt class="text-muted-foreground">Explorer PID</dt>
                <dd>{{ taskbarIdentity?.explorerProcessId ?? '读取中' }}</dd>
                <dt class="text-muted-foreground">DPI</dt>
                <dd>
                  {{
                    taskbarDpi
                      ? `${taskbarDpi.dpi}（${Math.round(taskbarDpi.scaleFactor * 100)}%）`
                      : '读取中'
                  }}
                </dd>
                <dt class="text-muted-foreground">物理尺寸</dt>
                <dd>
                  {{
                    taskbarDpi
                      ? `${taskbarDpi.physicalRect.width} × ${taskbarDpi.physicalRect.height} px`
                      : '读取中'
                  }}
                </dd>
                <dt class="text-muted-foreground">占用区来源</dt>
                <dd>
                  {{
                    taskbarOccupancy
                      ? taskbarOccupancySourceLabels[taskbarOccupancy.source]
                      : '读取中'
                  }}
                </dd>
                <dt class="text-muted-foreground">占用区数量</dt>
                <dd>{{ taskbarOccupancy?.regions.length ?? '读取中' }}</dd>
                <dt class="text-muted-foreground">碰撞状态</dt>
                <dd>未启用（按阶段 9.5 的简化定位规则）</dd>
              </dl>
              <Alert v-if="taskbarOccupancy?.fallbackReason">
                <InfoIcon />
                <AlertTitle>已使用 Win32 回退</AlertTitle>
                <AlertDescription>{{ taskbarOccupancy.fallbackReason }}</AlertDescription>
              </Alert>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>最近错误</CardTitle>
              <CardDescription>本次设置窗口运行期间捕获的最近错误。</CardDescription>
            </CardHeader>
            <CardContent>
              <p v-if="recentErrors.length === 0" class="text-muted-foreground text-sm">
                当前没有记录到错误。
              </p>
              <ul v-else class="flex flex-col gap-2">
                <li
                  v-for="error in recentErrors"
                  :key="error"
                  class="text-destructive rounded-lg border p-3 text-sm"
                >
                  {{ error }}
                </li>
              </ul>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>关于 Muse Bar</CardTitle>
              <CardDescription>Windows 11 任务栏系统媒体工具。</CardDescription>
            </CardHeader>
            <CardContent class="text-muted-foreground flex flex-col gap-2 text-sm">
              <p>当前版本聚焦主显示器、Child 真嵌入和 Windows SMTC 媒体会话。</p>
              <p>歌词、Owner 回退、多屏同步和自动更新不属于当前首版范围。</p>
            </CardContent>
          </Card>
        </template>
      </main>
    </SidebarInset>
  </SidebarProvider>
</template>
