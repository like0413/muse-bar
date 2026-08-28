<script setup lang="ts">
import { computed } from 'vue'

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { readColorMode, type ColorMode } from '@/lib/settings-api'

import type {
  AppearanceSettingsCardEmits,
  AppearanceSettingsCardProps,
} from './appearance-settings-contracts'

const props = defineProps<AppearanceSettingsCardProps>()
const emit = defineEmits<AppearanceSettingsCardEmits>()

const currentColorMode = computed(() => readColorMode(props.settings))
const colorModeOptions: ReadonlyArray<{ value: ColorMode; label: string }> = [
  { value: 'system', label: '跟随系统' },
  { value: 'dark', label: '深色' },
  { value: 'light', label: '浅色' },
]

function handleColorModeChange(colorMode: unknown): void {
  if (
    (colorMode === 'system' || colorMode === 'dark' || colorMode === 'light') &&
    colorMode !== currentColorMode.value
  )
    emit('change', { colorMode })
}
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle>颜色模式</CardTitle>
      <CardDescription>跟随 Windows，或固定 Muse Bar 的明暗主题。</CardDescription>
    </CardHeader>
    <CardContent>
      <ToggleGroup
        type="single"
        variant="outline"
        :disabled="disabled"
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
</template>
