<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { computed } from 'vue'

import { useSettingsStore } from '../settings-store'
import AppearanceAlignmentCard from './AppearanceAlignmentCard.vue'
import AppearanceColorModeCard from './AppearanceColorModeCard.vue'
import AppearanceControlsCard from './AppearanceControlsCard.vue'
import AppearanceProgressCard from './AppearanceProgressCard.vue'
import AppearanceTitleScrollCard from './AppearanceTitleScrollCard.vue'

const settingsStore = useSettingsStore()
const { settings, mediaSnapshot } = storeToRefs(settingsStore)
const controlsDisabled = computed(() => !settings.value)
</script>

<template>
  <div class="flex flex-col gap-4">
    <AppearanceColorModeCard
      :settings="settings"
      :disabled="controlsDisabled"
      @change="settingsStore.saveSettingsPatch"
    />
    <AppearanceControlsCard
      :settings="settings"
      :disabled="controlsDisabled"
      @change="settingsStore.saveSettingsPatch"
    />
    <AppearanceAlignmentCard
      :settings="settings"
      :disabled="controlsDisabled"
      @change="settingsStore.saveSettingsPatch"
    />
    <AppearanceProgressCard
      :settings="settings"
      :disabled="controlsDisabled"
      :media-snapshot="mediaSnapshot"
      @change="settingsStore.saveSettingsPatch"
    />
    <AppearanceTitleScrollCard
      :settings="settings"
      :disabled="controlsDisabled"
      @change="settingsStore.saveSettingsPatch"
    />
  </div>
</template>
