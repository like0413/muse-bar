<script setup lang="ts">
import { onBeforeUnmount, onMounted } from 'vue'

import { openSettingsWindow } from '@/lib/settings-window'
import { getErrorMessage } from '@/lib/utils'

import { useBarStore } from './bar-store'
import BarLayout from './components/BarLayout.vue'

const barStore = useBarStore()

/** 打开设置窗口，并将失败原因交给歌曲信息组件统一展示。 */
async function handleOpenSettings(): Promise<void> {
  barStore.setSettingsWindowError('')
  try {
    await openSettingsWindow()
  } catch (error) {
    barStore.setSettingsWindowError(`设置页打开失败：${getErrorMessage(error)}`)
  }
}

onMounted(() => void barStore.start())
onBeforeUnmount(barStore.stop)
</script>

<template>
  <BarLayout @open-settings="handleOpenSettings" />
</template>
