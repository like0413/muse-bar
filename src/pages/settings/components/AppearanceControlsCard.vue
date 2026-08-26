<script setup lang="ts">
import { computed } from 'vue'

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Field, FieldContent, FieldDescription, FieldTitle } from '@/components/ui/field'
import { Switch } from '@/components/ui/switch'
import { readShowControls } from '@/lib/settings-api'

import type {
  AppearanceSettingsCardEmits,
  AppearanceSettingsCardProps,
} from './appearance-settings-contracts'

const props = defineProps<AppearanceSettingsCardProps>()
const emit = defineEmits<AppearanceSettingsCardEmits>()
const showControls = computed(() => readShowControls(props.settings))

function handleShowControlsChange(show: boolean): void {
  if (show !== showControls.value) emit('change', { showControls: show })
}
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle>控制按钮</CardTitle>
      <CardDescription>控制上一曲、播放/暂停和下一曲按钮是否出现。</CardDescription>
    </CardHeader>
    <CardContent>
      <Field orientation="horizontal" :data-disabled="disabled">
        <FieldContent>
          <FieldTitle>显示控制按钮</FieldTitle>
          <FieldDescription>包括上一曲、播放/暂停和下一曲。</FieldDescription>
        </FieldContent>
        <Switch
          :model-value="showControls"
          :disabled="disabled"
          aria-label="显示控制按钮"
          @update:model-value="handleShowControlsChange"
        />
      </Field>
    </CardContent>
  </Card>
</template>
