<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { computed } from 'vue'

import type { SettingsPatch } from '@/lib/settings-api'

import { useSettingsStore } from '../settings-store'
import AppearanceAlignmentCard from './AppearanceAlignmentCard.vue'
import AppearanceColorModeCard from './AppearanceColorModeCard.vue'
import AppearanceControlsCard from './AppearanceControlsCard.vue'
import AppearanceProgressCard from './AppearanceProgressCard.vue'
import AppearanceTitleScrollCard from './AppearanceTitleScrollCard.vue'

const settingsStore = useSettingsStore()
const { settings, mediaSnapshot } = storeToRefs(settingsStore)
const controlsDisabled = computed(() => !settings.value)

function saveSettingsPatch(patch: SettingsPatch): void {
  void settingsStore.saveSettingsPatch(patch)
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <AppearanceColorModeCard
      :settings="settings"
      :disabled="controlsDisabled"
      @change="saveSettingsPatch"
    />
    <AppearanceControlsCard
      :settings="settings"
      :disabled="controlsDisabled"
      @change="saveSettingsPatch"
    />
    <AppearanceAlignmentCard
      :settings="settings"
      :disabled="controlsDisabled"
      @change="saveSettingsPatch"
    />
    <AppearanceProgressCard
      :settings="settings"
      :disabled="controlsDisabled"
      :media-snapshot="mediaSnapshot"
      @change="saveSettingsPatch"
    />
    <AppearanceTitleScrollCard
      :settings="settings"
      :disabled="controlsDisabled"
      @change="saveSettingsPatch"
    />
  </div>
</template>
