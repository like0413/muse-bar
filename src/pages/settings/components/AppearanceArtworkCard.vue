<script setup lang="ts">
import { computed } from 'vue'

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldTitle,
} from '@/components/ui/field'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { readArtworkShape, readRotateCircularArtwork, type ArtworkShape } from '@/lib/settings-api'

import type {
  AppearanceSettingsCardEmits,
  AppearanceSettingsCardProps,
} from './appearance-settings-contracts'

const props = defineProps<AppearanceSettingsCardProps>()
const emit = defineEmits<AppearanceSettingsCardEmits>()
const artworkShape = computed(() => readArtworkShape(props.settings))
const rotateCircularArtwork = computed(() => readRotateCircularArtwork(props.settings))
const artworkShapeOptions: ReadonlyArray<{ value: ArtworkShape; label: string }> = [
  { value: 'rounded', label: '方形' },
  { value: 'circle', label: '圆形' },
]

function handleArtworkShapeChange(shape: unknown): void {
  if ((shape === 'rounded' || shape === 'circle') && shape !== artworkShape.value)
    emit('change', { artworkShape: shape })
}

function handleArtworkRotationChange(rotate: boolean): void {
  if (rotate !== rotateCircularArtwork.value) emit('change', { rotateCircularArtwork: rotate })
}
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle>歌曲封面</CardTitle>
      <CardDescription>调整 Bar 中封面的形状与播放动画。</CardDescription>
    </CardHeader>
    <CardContent>
      <FieldGroup>
        <Field :data-disabled="disabled">
          <FieldLabel>封面形状</FieldLabel>
          <ToggleGroup
            type="single"
            variant="outline"
            :model-value="artworkShape"
            :disabled="disabled"
            @update:model-value="handleArtworkShapeChange"
          >
            <ToggleGroupItem
              v-for="option in artworkShapeOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </ToggleGroupItem>
          </ToggleGroup>
        </Field>
        <Field orientation="horizontal" :data-disabled="disabled || artworkShape !== 'circle'">
          <FieldContent>
            <FieldTitle>旋转圆形封面</FieldTitle>
            <FieldDescription>播放时缓慢旋转，暂停时停在当前位置。</FieldDescription>
          </FieldContent>
          <Switch
            :model-value="rotateCircularArtwork"
            :disabled="disabled || artworkShape !== 'circle'"
            aria-label="旋转圆形封面"
            @update:model-value="handleArtworkRotationChange"
          />
        </Field>
      </FieldGroup>
    </CardContent>
  </Card>
</template>
