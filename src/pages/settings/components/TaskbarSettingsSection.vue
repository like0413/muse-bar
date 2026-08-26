<script setup lang="ts">
import { AlertCircleIcon } from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed, shallowRef, watch } from 'vue'

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
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
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import {
  readLyricsAlignment,
  readLyricsEnabled,
  readManualOffset,
  readMaximumWidth,
  readTargetMonitor,
  readTaskbarPosition,
  readWindowMode,
  type LyricsAlignment,
  type TaskbarPosition,
} from '@/lib/settings-api'

import { useSettingsStore } from '../settings-store'

const MAXIMUM_WIDTH_SLIDER_MINIMUM = 200
const MAXIMUM_WIDTH_SLIDER_MAXIMUM = 520
const WIDTH_SLIDER_STEP = 4

const settingsStore = useSettingsStore()
const { settings, taskbarMonitors, taskbarMonitorError } = storeToRefs(settingsStore)
const maxWidthDraft = shallowRef<number[]>([])
const manualOffsetDraft = shallowRef('0')

const currentPosition = computed(() => readTaskbarPosition(settings.value))
const currentTargetMonitor = computed(() => readTargetMonitor(settings.value))
const targetMonitorSelection = computed(() => {
  const selected = currentTargetMonitor.value
  return taskbarMonitors.value.some((monitor) => monitor.id === selected) ? selected : 'primary'
})
const lyricsEnabled = computed(() => readLyricsEnabled(settings.value))
const currentLyricsAlignment = computed(() => readLyricsAlignment(settings.value))
const currentWindowMode = computed(() => readWindowMode(settings.value))

const positionOptions: ReadonlyArray<{ value: TaskbarPosition; label: string }> = [
  { value: 'left', label: '靠左' },
  { value: 'right', label: '靠右' },
]
const lyricsAlignmentOptions: ReadonlyArray<{ value: LyricsAlignment; label: string }> = [
  { value: 'left', label: '左对齐' },
  { value: 'center', label: '居中' },
  { value: 'right', label: '右对齐' },
]

/** 保存合法的任务栏位置。 */
function handlePositionChange(position: unknown): void {
  if ((position === 'left' || position === 'right') && position !== currentPosition.value)
    void settingsStore.saveSettingsPatch({ position })
}

/** 保存目标任务栏显示器的设备标识。 */
function handleTargetMonitorChange(targetMonitor: unknown): void {
  if (
    typeof targetMonitor === 'string' &&
    targetMonitor &&
    targetMonitor !== currentTargetMonitor.value
  ) {
    void settingsStore.saveSettingsPatch({ targetMonitor })
  }
}

/** 更新手动偏移草稿，提交前不移动原生窗口。 */
function handleManualOffsetDraftChange(value: string | number): void {
  manualOffsetDraft.value = String(value)
}

/** 保存 -200 到 200 之间的整数偏移。 */
function commitManualOffset(): void {
  const parsed = Number(manualOffsetDraft.value)
  if (!Number.isFinite(parsed)) return
  const manualOffset = Math.round(Math.min(200, Math.max(-200, parsed)))
  manualOffsetDraft.value = String(manualOffset)
  if (manualOffset !== readManualOffset(settings.value))
    void settingsStore.saveSettingsPatch({ manualOffset })
}

/** 保存歌词模式开关。 */
function handleLyricsEnabledChange(enabled: boolean): void {
  if (enabled !== lyricsEnabled.value)
    void settingsStore.saveSettingsPatch({ lyricsEnabled: enabled })
}

/** 保存合法的歌词水平对齐方式。 */
function handleLyricsAlignmentChange(alignment: unknown): void {
  if (
    (alignment === 'left' || alignment === 'center' || alignment === 'right') &&
    alignment !== currentLyricsAlignment.value
  ) {
    void settingsStore.saveSettingsPatch({ lyricsAlignment: alignment })
  }
}

/** 更新普通模式最大宽度草稿。 */
function handleMaximumWidthDraftChange(value: number[] | undefined): void {
  const width = value?.[0]
  if (width !== undefined) maxWidthDraft.value = [width]
}

/** 在拖动结束后提交普通模式最大宽度。 */
function commitMaximumWidth(value: number[]): void {
  handleMaximumWidthDraftChange(value)
  const width = maxWidthDraft.value[0]
  if (width !== undefined) void settingsStore.saveSettingsPatch({ maxWidth: width })
}

watch(
  settings,
  (value) => {
    const maximumWidth = readMaximumWidth(value)
    const manualOffset = readManualOffset(value)
    if (maximumWidth !== undefined) maxWidthDraft.value = [maximumWidth]
    if (manualOffset !== undefined) manualOffsetDraft.value = String(manualOffset)
  },
  { immediate: true },
)
</script>

<template>
  <div class="flex flex-col gap-4">
    <Alert v-if="taskbarMonitorError" variant="destructive">
      <AlertCircleIcon />
      <AlertTitle>显示器列表读取失败</AlertTitle>
      <AlertDescription>{{ taskbarMonitorError }}</AlertDescription>
    </Alert>

    <Card>
      <CardHeader>
        <CardTitle>位置与尺寸</CardTitle>
        <CardDescription>控制 Bar 所在的任务栏、位置和普通模式最大宽度。</CardDescription>
      </CardHeader>
      <CardContent>
        <FieldGroup>
          <Field>
            <FieldLabel>目标显示器</FieldLabel>
            <Select
              :model-value="targetMonitorSelection"
              :disabled="!settings || taskbarMonitors.length === 0"
              @update:model-value="handleTargetMonitorChange"
            >
              <SelectTrigger class="w-full">
                <SelectValue placeholder="选择具有任务栏的显示器" />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem
                    v-for="monitor in taskbarMonitors"
                    :key="monitor.id"
                    :value="monitor.id"
                  >
                    {{ monitor.label }}
                  </SelectItem>
                </SelectGroup>
              </SelectContent>
            </Select>
            <FieldDescription>只列出当前具有 Windows 任务栏的显示器。</FieldDescription>
          </Field>

          <Field>
            <FieldLabel>任务栏位置</FieldLabel>
            <ToggleGroup
              type="single"
              variant="outline"
              :disabled="!settings"
              :model-value="currentPosition"
              @update:model-value="handlePositionChange"
            >
              <ToggleGroupItem
                v-for="option in positionOptions"
                :key="option.value"
                :value="option.value"
              >
                {{ option.label }}
              </ToggleGroupItem>
            </ToggleGroup>
            <FieldDescription>Bar 会紧贴所选一侧的任务栏组件。</FieldDescription>
          </Field>

          <Field>
            <div class="flex items-center justify-between gap-4">
              <FieldLabel>普通模式最大宽度</FieldLabel>
              <Badge variant="outline">
                {{ maxWidthDraft[0] ?? '读取中'
                }}<template v-if="maxWidthDraft[0] !== undefined"> px</template>
              </Badge>
            </div>
            <Slider
              aria-label="Bar 普通模式最大宽度"
              :model-value="maxWidthDraft"
              :min="MAXIMUM_WIDTH_SLIDER_MINIMUM"
              :max="MAXIMUM_WIDTH_SLIDER_MAXIMUM"
              :step="WIDTH_SLIDER_STEP"
              :disabled="!settings"
              @update:model-value="handleMaximumWidthDraftChange"
              @value-commit="commitMaximumWidth"
            />
            <FieldDescription>
              普通模式按内容自然收缩；歌词模式占满对应任务栏空白区域。
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
              :disabled="!settings"
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
              :disabled="!settings"
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
              :disabled="!settings || !lyricsEnabled"
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
            <FieldDescription>控制歌词在封面旁可用区域中的水平位置。</FieldDescription>
          </Field>
        </FieldGroup>
      </CardContent>
    </Card>
  </div>
</template>
