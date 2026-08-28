<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue'

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
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import {
  readTitleScrollEnabled,
  readTitleScrollMode,
  readTitleScrollSpeed,
  type TitleScrollMode,
} from '@/lib/settings-api'

import type {
  AppearanceSettingsCardEmits,
  AppearanceSettingsCardProps,
} from './appearance-settings-contracts'

const TITLE_SCROLL_SPEED_MINIMUM = 10
const TITLE_SCROLL_SPEED_MAXIMUM = 100
const TITLE_SCROLL_SPEED_STEP = 5

const props = defineProps<AppearanceSettingsCardProps>()
const emit = defineEmits<AppearanceSettingsCardEmits>()
const speedDraft = shallowRef<number[]>([])
const titleScrollEnabled = computed(() => readTitleScrollEnabled(props.settings))
const currentTitleScrollMode = computed(() => readTitleScrollMode(props.settings))
const titleScrollModeOptions: ReadonlyArray<{ value: TitleScrollMode; label: string }> = [
  { value: 'continuous', label: '连续滚动' },
  { value: 'restart', label: '从头滚动' },
  { value: 'bounce', label: '来回滚动' },
]

function handleEnabledChange(enabled: boolean): void {
  if (enabled !== titleScrollEnabled.value) emit('change', { titleScrollEnabled: enabled })
}

function handleModeChange(mode: unknown): void {
  if (
    (mode === 'continuous' || mode === 'restart' || mode === 'bounce') &&
    mode !== currentTitleScrollMode.value
  )
    emit('change', { titleScrollMode: mode })
}

function handleSpeedDraftChange(value: number[] | undefined): void {
  const speed = value?.[0]
  if (speed !== undefined) speedDraft.value = [speed]
}

function commitSpeed(value: number[]): void {
  handleSpeedDraftChange(value)
  const speed = speedDraft.value[0]
  if (speed !== undefined) emit('change', { titleScrollSpeed: speed })
}

watch(
  () => props.settings,
  (settings) => {
    speedDraft.value = [readTitleScrollSpeed(settings)]
  },
  { immediate: true },
)
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle>滚动文本</CardTitle>
      <CardDescription>歌曲名超过可用宽度时自动滚动，短标题始终保持静止。</CardDescription>
    </CardHeader>
    <CardContent>
      <FieldGroup>
        <Field orientation="horizontal" :data-disabled="disabled">
          <FieldContent>
            <FieldTitle>滚动长标题</FieldTitle>
            <FieldDescription>只影响歌曲名，不滚动歌手信息。</FieldDescription>
          </FieldContent>
          <Switch
            :model-value="titleScrollEnabled"
            :disabled="disabled"
            aria-label="滚动长标题"
            @update:model-value="handleEnabledChange"
          />
        </Field>
        <Field :data-disabled="disabled || !titleScrollEnabled">
          <div class="flex items-center justify-between gap-4">
            <FieldLabel>滚动速度</FieldLabel>
            <Badge variant="outline">
              {{ speedDraft[0] ?? '读取中'
              }}<template v-if="speedDraft[0] !== undefined"> px/s</template>
            </Badge>
          </div>
          <Slider
            aria-label="标题滚动速度"
            :model-value="speedDraft"
            :min="TITLE_SCROLL_SPEED_MINIMUM"
            :max="TITLE_SCROLL_SPEED_MAXIMUM"
            :step="TITLE_SCROLL_SPEED_STEP"
            :disabled="disabled || !titleScrollEnabled"
            @update:model-value="handleSpeedDraftChange"
            @value-commit="commitSpeed"
          />
          <FieldDescription>数值越大，标题每秒移动的距离越远。</FieldDescription>
        </Field>
        <Field :data-disabled="disabled || !titleScrollEnabled">
          <FieldLabel>滚动方式</FieldLabel>
          <ToggleGroup
            type="single"
            variant="outline"
            :model-value="currentTitleScrollMode"
            :disabled="disabled || !titleScrollEnabled"
            @update:model-value="handleModeChange"
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
</template>
