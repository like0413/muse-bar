<script setup lang="ts">
import { computed } from 'vue'

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Field, FieldLabel } from '@/components/ui/field'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { readElementAlignment, type ElementAlignment } from '@/lib/settings-api'

import type {
  AppearanceSettingsCardEmits,
  AppearanceSettingsCardProps,
} from './appearance-settings-contracts'

const props = defineProps<AppearanceSettingsCardProps>()
const emit = defineEmits<AppearanceSettingsCardEmits>()
const currentAlignment = computed(() => readElementAlignment(props.settings))
const alignmentOptions: ReadonlyArray<{ value: ElementAlignment; label: string }> = [
  { value: 'left', label: '居左' },
  { value: 'right', label: '居右' },
]

function handleAlignmentChange(alignment: unknown): void {
  if ((alignment === 'left' || alignment === 'right') && alignment !== currentAlignment.value)
    emit('change', { elementAlignment: alignment })
}
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle>元素对齐</CardTitle>
      <CardDescription>
        居右时整体镜像排列，封面位于最右侧，媒体文字和歌词向左展开。
      </CardDescription>
    </CardHeader>
    <CardContent>
      <Field :data-disabled="disabled">
        <FieldLabel>排列方向</FieldLabel>
        <ToggleGroup
          type="single"
          variant="outline"
          :model-value="currentAlignment"
          :disabled="disabled"
          @update:model-value="handleAlignmentChange"
        >
          <ToggleGroupItem
            v-for="option in alignmentOptions"
            :key="option.value"
            :value="option.value"
          >
            {{ option.label }}
          </ToggleGroupItem>
        </ToggleGroup>
      </Field>
    </CardContent>
  </Card>
</template>
