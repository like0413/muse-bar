<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { computed } from 'vue'

import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldTitle,
} from '@/components/ui/field'
import { Switch } from '@/components/ui/switch'
import { readLaunchOnStartup } from '@/lib/settings-api'

import { useSettingsStore } from '../settings-store'

const settingsStore = useSettingsStore()
const { settings, isSavingSettings } = storeToRefs(settingsStore)
const launchOnStartup = computed(() => readLaunchOnStartup(settings.value))

/** 保存开机启动开关，Rust 会同步 Windows 当前用户启动项。 */
function handleLaunchOnStartupChange(enabled: boolean): void {
  if (enabled !== launchOnStartup.value)
    void settingsStore.saveSettingsPatch({ launchOnStartup: enabled })
}
</script>

<template>
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
            <FieldDescription>使用 Windows 当前用户启动项，不需要管理员权限。</FieldDescription>
          </FieldContent>
          <Switch
            :model-value="launchOnStartup"
            :disabled="isSavingSettings || !settings"
            aria-label="开机启动"
            @update:model-value="handleLaunchOnStartupChange"
          />
        </Field>
      </FieldGroup>
    </CardContent>
  </Card>
</template>
