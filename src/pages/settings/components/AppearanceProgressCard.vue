<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue'

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
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import type { MediaSnapshot } from '@/lib/media-types'
import {
  readArtworkShape,
  readCustomProgressColor,
  readElementAlignment,
  readProgressColorSource,
  readProgressStyle,
  readShowControls,
  readShowProgress,
  readRotateCircularArtwork,
  readSpectrumMode,
  readSpectrumOrigin,
  type ProgressColorSource,
  type ProgressStyle,
  type SpectrumMode,
  type SpectrumOrigin,
} from '@/lib/settings-api'

import type {
  AppearanceSettingsCardEmits,
  AppearanceSettingsCardProps,
} from './appearance-settings-contracts'
import BarAppearancePreview from './BarAppearancePreview.vue'

interface Props extends AppearanceSettingsCardProps {
  mediaSnapshot: MediaSnapshot | null
}

const props = defineProps<Props>()
const emit = defineEmits<AppearanceSettingsCardEmits>()
const customColorDraft = shallowRef('#0078D4')

const showProgress = computed(() => readShowProgress(props.settings))
const spectrumMode = computed(() => readSpectrumMode(props.settings))
const spectrumOrigin = computed(() => readSpectrumOrigin(props.settings))
const spectrumEnabled = computed(() => spectrumMode.value !== 'off')
const usesAccentColor = computed(() => showProgress.value || spectrumEnabled.value)
const currentProgressStyle = computed(() => readProgressStyle(props.settings))
const currentColorSource = computed(() => readProgressColorSource(props.settings))
const isCustomColorValid = computed(() => /^#[0-9a-f]{6}$/i.test(customColorDraft.value.trim()))
const previewAccentColor = computed(() => {
  if (currentColorSource.value === 'custom') return readCustomProgressColor(props.settings)
  if (currentColorSource.value === 'system')
    return props.mediaSnapshot?.systemAccentColor || '#0078D4'
  return props.mediaSnapshot?.accentColor || '#0078D4'
})
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
const spectrumModeOptions: ReadonlyArray<{ value: SpectrumMode; label: string }> = [
  { value: 'off', label: '关闭' },
  { value: 'economy', label: '省电 20 FPS' },
  { value: 'smooth', label: '流畅 30 FPS' },
]
const spectrumOriginOptions: ReadonlyArray<{ value: SpectrumOrigin; label: string }> = [
  { value: 'bottom', label: '普通' },
  { value: 'center', label: '居中' },
]
const customColorPresets = [
  '#0078D4',
  '#00B7C3',
  '#107C10',
  '#6B69D6',
  '#C239B3',
  '#E74856',
  '#F7630C',
  '#FFB900',
] as const

function handleShowProgressChange(show: boolean): void {
  if (show !== showProgress.value) emit('change', { showProgress: show })
}

function handleSpectrumModeChange(mode: unknown): void {
  if ((mode === 'off' || mode === 'economy' || mode === 'smooth') && mode !== spectrumMode.value)
    emit('change', { spectrumMode: mode })
}

function handleSpectrumOriginChange(origin: unknown): void {
  if ((origin === 'bottom' || origin === 'center') && origin !== spectrumOrigin.value)
    emit('change', { spectrumOrigin: origin })
}

function handleProgressStyleChange(progressStyle: unknown): void {
  if (
    (progressStyle === 'underline' || progressStyle === 'background-gradient') &&
    progressStyle !== currentProgressStyle.value
  )
    emit('change', { progressStyle })
}

function handleColorSourceChange(source: unknown): void {
  if (
    (source === 'artwork' || source === 'system' || source === 'custom') &&
    source !== currentColorSource.value
  )
    emit('change', { progressColorSource: source })
}

function handleColorPresetChange(color: unknown): void {
  if (typeof color !== 'string' || !/^#[0-9a-f]{6}$/i.test(color)) return
  customColorDraft.value = color.toUpperCase()
  emit('change', { customProgressColor: customColorDraft.value })
}

function handleColorDraftChange(value: string | number): void {
  customColorDraft.value = String(value)
}

function commitCustomColor(): void {
  const color = customColorDraft.value.trim().toUpperCase()
  if (!/^#[0-9A-F]{6}$/.test(color)) return
  customColorDraft.value = color
  if (color !== readCustomProgressColor(props.settings))
    emit('change', { customProgressColor: color })
}

watch(
  () => props.settings,
  (settings) => {
    customColorDraft.value = readCustomProgressColor(settings)
  },
  { immediate: true },
)
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle>播放视觉</CardTitle>
      <CardDescription>控制播放进度、实时频谱及它们共用的强调色。</CardDescription>
    </CardHeader>
    <CardContent class="flex flex-col gap-6">
      <FieldGroup>
        <Field orientation="horizontal" :data-disabled="disabled">
          <FieldContent>
            <FieldTitle>显示播放进度</FieldTitle>
            <FieldDescription>关闭后不会显示底线或背景渐变。</FieldDescription>
          </FieldContent>
          <Switch
            :model-value="showProgress"
            :disabled="disabled"
            aria-label="显示播放进度"
            @update:model-value="handleShowProgressChange"
          />
        </Field>
        <Field :data-disabled="disabled">
          <FieldLabel>实时频谱</FieldLabel>
          <ToggleGroup
            type="single"
            variant="outline"
            :model-value="spectrumMode"
            :disabled="disabled"
            @update:model-value="handleSpectrumModeChange"
          >
            <ToggleGroupItem
              v-for="option in spectrumModeOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </ToggleGroupItem>
          </ToggleGroup>
          <FieldDescription>
            分析当前播放器音频并在独立区域显示频谱；关闭时不会采集音频。
          </FieldDescription>
        </Field>
        <Field :data-disabled="disabled || !spectrumEnabled">
          <FieldLabel>频谱样式</FieldLabel>
          <ToggleGroup
            type="single"
            variant="outline"
            :model-value="spectrumOrigin"
            :disabled="disabled || !spectrumEnabled"
            @update:model-value="handleSpectrumOriginChange"
          >
            <ToggleGroupItem
              v-for="option in spectrumOriginOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </ToggleGroupItem>
          </ToggleGroup>
          <FieldDescription>
            普通模式从容器底部向上增长；居中模式从中线同时向上下增长。
          </FieldDescription>
        </Field>
        <Field :data-disabled="disabled || !showProgress">
          <FieldLabel>进度样式</FieldLabel>
          <ToggleGroup
            type="single"
            variant="outline"
            :disabled="disabled || !showProgress"
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
        <Field :data-disabled="disabled || !usesAccentColor">
          <FieldLabel>视觉颜色</FieldLabel>
          <ToggleGroup
            type="single"
            variant="outline"
            :model-value="currentColorSource"
            :disabled="disabled || !usesAccentColor"
            @update:model-value="handleColorSourceChange"
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
          v-if="currentColorSource === 'custom'"
          :data-disabled="disabled || !usesAccentColor"
          :data-invalid="!isCustomColorValid"
        >
          <FieldLabel>自定义颜色</FieldLabel>
          <ToggleGroup
            type="single"
            variant="outline"
            class="flex-wrap justify-start"
            :model-value="readCustomProgressColor(settings)"
            :disabled="disabled || !usesAccentColor"
            @update:model-value="handleColorPresetChange"
          >
            <ToggleGroupItem
              v-for="color in customColorPresets"
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
            :model-value="customColorDraft"
            placeholder="#0078D4"
            maxlength="7"
            spellcheck="false"
            :aria-invalid="!isCustomColorValid"
            :disabled="disabled || !usesAccentColor"
            @update:model-value="handleColorDraftChange"
            @blur="commitCustomColor"
            @keydown.enter="commitCustomColor"
          />
          <FieldDescription>输入“#”加六位十六进制颜色，例如 #FF5A5F。</FieldDescription>
        </Field>
      </FieldGroup>

      <BarAppearancePreview
        :media-snapshot="mediaSnapshot"
        :artwork-shape="readArtworkShape(settings)"
        :rotate-circular-artwork="readRotateCircularArtwork(settings)"
        :accent-color="previewAccentColor"
        :alignment="readElementAlignment(settings)"
        :show-controls="readShowControls(settings)"
        :show-progress="showProgress"
        :spectrum-enabled="spectrumEnabled"
        :spectrum-origin="spectrumOrigin"
        :progress-style="currentProgressStyle"
      />
    </CardContent>
  </Card>
</template>
