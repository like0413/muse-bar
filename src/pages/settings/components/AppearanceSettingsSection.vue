<script setup lang="ts">
import { Music2Icon } from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed, shallowRef, watch } from 'vue'

import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
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
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import {
  readColorMode,
  readCustomProgressColor,
  readElementAlignment,
  readProgressColorSource,
  readProgressStyle,
  readShowControls,
  readShowProgress,
  readTitleScrollEnabled,
  readTitleScrollMode,
  readTitleScrollSpeed,
  type ColorMode,
  type ElementAlignment,
  type ProgressColorSource,
  type ProgressStyle,
  type TitleScrollMode,
} from '@/lib/settings-api'
import { cn } from '@/lib/utils'

import { useSettingsStore } from '../settings-store'

const TITLE_SCROLL_SPEED_MINIMUM = 10
const TITLE_SCROLL_SPEED_MAXIMUM = 100
const TITLE_SCROLL_SPEED_STEP = 5

const settingsStore = useSettingsStore()
const { settings, isSavingSettings, mediaSnapshot } = storeToRefs(settingsStore)
const customProgressColorDraft = shallowRef('#0078D4')
const titleScrollSpeedDraft = shallowRef<number[]>([])

const currentColorMode = computed(() => readColorMode(settings.value))
const showControls = computed(() => readShowControls(settings.value))
const currentElementAlignment = computed(() => readElementAlignment(settings.value))
const showProgress = computed(() => readShowProgress(settings.value))
const currentProgressStyle = computed(() => readProgressStyle(settings.value))
const currentProgressColorSource = computed(() => readProgressColorSource(settings.value))
const titleScrollEnabled = computed(() => readTitleScrollEnabled(settings.value))
const currentTitleScrollMode = computed(() => readTitleScrollMode(settings.value))
const isCustomProgressColorValid = computed(() =>
  /^#[0-9a-f]{6}$/i.test(customProgressColorDraft.value.trim()),
)
const previewAccentColor = computed(() => {
  if (currentProgressColorSource.value === 'custom') return readCustomProgressColor(settings.value)
  if (currentProgressColorSource.value === 'system')
    return mediaSnapshot.value?.systemAccentColor || '#0078D4'
  return mediaSnapshot.value?.accentColor || '#0078D4'
})
const previewLayoutClass = computed(() =>
  cn(
    'bg-card text-card-foreground relative flex h-14 w-full max-w-md items-center gap-3 overflow-hidden rounded-xl border px-3 shadow-sm',
    { 'flex-row-reverse': currentElementAlignment.value === 'right' },
  ),
)
const previewTextClass = computed(() =>
  cn('relative min-w-0 flex-1', {
    'text-right': currentElementAlignment.value === 'right',
  }),
)

const colorModeOptions: ReadonlyArray<{ value: ColorMode; label: string }> = [
  { value: 'system', label: '跟随系统' },
  { value: 'dark', label: '深色' },
  { value: 'light', label: '浅色' },
]
const elementAlignmentOptions: ReadonlyArray<{ value: ElementAlignment; label: string }> = [
  { value: 'left', label: '居左' },
  { value: 'right', label: '居右' },
]
const progressStyleOptions: ReadonlyArray<{ value: ProgressStyle; label: string }> = [
  { value: 'underline', label: '底部细线' },
  { value: 'background-gradient', label: '背景渐变' },
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

/** 保存合法的颜色模式。 */
function handleColorModeChange(colorMode: unknown): void {
  if (
    (colorMode === 'system' || colorMode === 'dark' || colorMode === 'light') &&
    colorMode !== currentColorMode.value
  )
    void settingsStore.saveSettingsPatch({ colorMode })
}

/** 保存控制按钮显隐状态。 */
function handleShowControlsChange(show: boolean): void {
  if (show !== showControls.value) void settingsStore.saveSettingsPatch({ showControls: show })
}

/** 保存整体元素排列方向。 */
function handleElementAlignmentChange(alignment: unknown): void {
  if (
    (alignment === 'left' || alignment === 'right') &&
    alignment !== currentElementAlignment.value
  )
    void settingsStore.saveSettingsPatch({ elementAlignment: alignment })
}

/** 保存进度视觉显隐状态。 */
function handleShowProgressChange(show: boolean): void {
  if (show !== showProgress.value) void settingsStore.saveSettingsPatch({ showProgress: show })
}

/** 保存合法的进度样式。 */
function handleProgressStyleChange(progressStyle: unknown): void {
  if (
    (progressStyle === 'underline' || progressStyle === 'background-gradient') &&
    progressStyle !== currentProgressStyle.value
  )
    void settingsStore.saveSettingsPatch({ progressStyle })
}

/** 保存合法的进度颜色来源。 */
function handleProgressColorSourceChange(source: unknown): void {
  if (
    (source === 'artwork' || source === 'system' || source === 'custom') &&
    source !== currentProgressColorSource.value
  )
    void settingsStore.saveSettingsPatch({ progressColorSource: source })
}

/** 选择预设颜色时立即保存，并同步手动输入框。 */
function handleCustomProgressColorPresetChange(color: unknown): void {
  if (typeof color !== 'string' || !/^#[0-9a-f]{6}$/i.test(color)) return
  customProgressColorDraft.value = color.toUpperCase()
  void settingsStore.saveSettingsPatch({ customProgressColor: customProgressColorDraft.value })
}

/** 更新自定义颜色输入草稿。 */
function handleCustomProgressColorDraftChange(value: string | number): void {
  customProgressColorDraft.value = String(value)
}

/** 保存合法的六位十六进制颜色。 */
function commitCustomProgressColor(): void {
  const color = customProgressColorDraft.value.trim().toUpperCase()
  if (!/^#[0-9A-F]{6}$/.test(color)) return
  customProgressColorDraft.value = color
  if (color !== readCustomProgressColor(settings.value))
    void settingsStore.saveSettingsPatch({ customProgressColor: color })
}

/** 保存标题滚动总开关。 */
function handleTitleScrollEnabledChange(enabled: boolean): void {
  if (enabled !== titleScrollEnabled.value)
    void settingsStore.saveSettingsPatch({ titleScrollEnabled: enabled })
}

/** 保存合法的标题滚动方式。 */
function handleTitleScrollModeChange(mode: unknown): void {
  if (
    (mode === 'continuous' || mode === 'restart' || mode === 'bounce') &&
    mode !== currentTitleScrollMode.value
  )
    void settingsStore.saveSettingsPatch({ titleScrollMode: mode })
}

/** 更新标题滚动速度草稿。 */
function handleTitleScrollSpeedDraftChange(value: number[] | undefined): void {
  const speed = value?.[0]
  if (speed !== undefined) titleScrollSpeedDraft.value = [speed]
}

/** 在拖动结束后提交标题滚动速度。 */
function commitTitleScrollSpeed(value: number[]): void {
  handleTitleScrollSpeedDraftChange(value)
  const speed = titleScrollSpeedDraft.value[0]
  if (speed !== undefined) void settingsStore.saveSettingsPatch({ titleScrollSpeed: speed })
}

watch(
  settings,
  (value) => {
    customProgressColorDraft.value = readCustomProgressColor(value)
    titleScrollSpeedDraft.value = [readTitleScrollSpeed(value)]
  },
  { immediate: true },
)
</script>

<template>
  <div class="flex flex-col gap-4">
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
          >
            {{ option.label }}
          </ToggleGroupItem>
        </ToggleGroup>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>控制按钮</CardTitle>
        <CardDescription>控制上一曲、播放/暂停和下一曲按钮是否出现。</CardDescription>
      </CardHeader>
      <CardContent>
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
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>元素对齐</CardTitle>
        <CardDescription>
          居右时整体镜像排列，封面位于最右侧，媒体文字和歌词向左展开。
        </CardDescription>
      </CardHeader>
      <CardContent>
        <Field>
          <FieldLabel>排列方向</FieldLabel>
          <ToggleGroup
            type="single"
            variant="outline"
            :model-value="currentElementAlignment"
            :disabled="isSavingSettings || !settings"
            @update:model-value="handleElementAlignmentChange"
          >
            <ToggleGroupItem
              v-for="option in elementAlignmentOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </ToggleGroupItem>
          </ToggleGroup>
        </Field>
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
            <FieldDescription>输入“#”加六位十六进制颜色，例如 #FF5A5F。</FieldDescription>
          </Field>
        </FieldGroup>

        <div class="bg-muted flex min-h-36 items-center justify-center rounded-xl border p-6">
          <div :class="previewLayoutClass">
            <div
              v-if="showProgress && currentProgressStyle === 'background-gradient'"
              class="pointer-events-none absolute inset-y-0 left-0 w-3/5"
              :style="{
                background: `linear-gradient(90deg, transparent, color-mix(in srgb, ${previewAccentColor} 42%, transparent))`,
              }"
            />
            <Avatar class="relative size-10 rounded-md">
              <AvatarImage
                v-if="mediaSnapshot?.artworkDataUrl"
                :src="mediaSnapshot.artworkDataUrl"
              />
              <AvatarFallback class="rounded-md"><Music2Icon /></AvatarFallback>
            </Avatar>
            <div :class="previewTextClass">
              <p class="truncate text-sm font-medium">
                {{ mediaSnapshot?.title || 'Muse Bar 预览' }}
              </p>
              <p class="text-muted-foreground truncate text-xs">
                {{ mediaSnapshot?.artist || '当前歌曲歌手' }}
              </p>
            </div>
            <div v-if="showControls" class="relative shrink-0 text-sm" aria-hidden="true">
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
              <Badge variant="outline">
                {{ titleScrollSpeedDraft[0] ?? '读取中'
                }}<template v-if="titleScrollSpeedDraft[0] !== undefined"> px/s</template>
              </Badge>
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
              >
                {{ option.label }}
              </ToggleGroupItem>
            </ToggleGroup>
            <FieldDescription>
              连续滚动会首尾衔接；从头滚动会在末尾重置；来回滚动会在两端之间往返。
            </FieldDescription>
          </Field>
        </FieldGroup>
      </CardContent>
    </Card>
  </div>
</template>
