<script setup lang="ts">
import { AlertCircleIcon } from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef } from 'vue'

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { SidebarInset, SidebarProvider } from '@/components/ui/sidebar'
import { waitForColorModeReady } from '@/lib/color-mode'
import { showReadySettingsWindow } from '@/lib/settings-window'

import AppearanceSettingsSection from './components/AppearanceSettingsSection.vue'
import DiagnosticsSettingsSection from './components/DiagnosticsSettingsSection.vue'
import GeneralSettingsSection from './components/GeneralSettingsSection.vue'
import MediaSettingsSection from './components/MediaSettingsSection.vue'
import SettingsHeader from './components/SettingsHeader.vue'
import SettingsSidebar from './components/SettingsSidebar.vue'
import TaskbarSettingsSection from './components/TaskbarSettingsSection.vue'
import { getSettingsNavigationItem, type SettingsSection } from './settings-navigation'
import { useSettingsStore } from './settings-store'

const activeSection = shallowRef<SettingsSection>('taskbar')
const settingsStore = useSettingsStore()
const { settingsError } = storeToRefs(settingsStore)
let hasUnmounted = false

const activeSectionComponent = computed(() => {
  const components = {
    taskbar: TaskbarSettingsSection,
    appearance: AppearanceSettingsSection,
    media: MediaSettingsSection,
    general: GeneralSettingsSection,
    diagnostics: DiagnosticsSettingsSection,
  } as const
  return components[activeSection.value]
})

/** 等待浏览器至少绘制一次，同时避免隐藏窗口中动画帧暂停造成永久等待。 */
function waitForInitialPaint(): Promise<void> {
  return new Promise((resolve) => {
    let resolved = false
    const finish = () => {
      if (resolved) return
      resolved = true
      resolve()
    }

    window.requestAnimationFrame(() => window.requestAnimationFrame(finish))
    window.setTimeout(finish, 50)
  })
}

/** 数据、主题和首屏内容全部准备好后再显示原生设置窗口。 */
async function initializeSettingsPage(): Promise<void> {
  await Promise.all([settingsStore.start(), waitForColorModeReady()])
  if (hasUnmounted) return

  await nextTick()
  await waitForInitialPaint()
  if (hasUnmounted) return

  try {
    await showReadySettingsWindow()
  } catch (error) {
    console.error('设置页准备完成，但无法显示原生窗口：', error)
  }
}

onMounted(() => void initializeSettingsPage())
onBeforeUnmount(() => {
  hasUnmounted = true
  settingsStore.stop()
})
</script>

<template>
  <SidebarProvider class="h-svh min-h-0 overflow-hidden">
    <SettingsSidebar v-model="activeSection" />
    <SidebarInset class="min-h-0 min-w-0 overflow-hidden">
      <SettingsHeader :active-section="activeSection" />
      <main class="settings-scroll-area flex min-h-0 flex-1 flex-col overflow-y-auto p-4 pt-0">
        <div class="mx-auto flex w-full max-w-4xl flex-col gap-4 pb-4">
          <div class="px-1 py-2">
            <h1 class="text-2xl font-semibold tracking-tight">
              {{ getSettingsNavigationItem(activeSection).label }}
            </h1>
            <p class="text-muted-foreground mt-1 text-sm">
              {{ getSettingsNavigationItem(activeSection).description }}
            </p>
          </div>

          <Alert v-if="settingsError" variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>设置保存失败</AlertTitle>
            <AlertDescription>{{ settingsError }}</AlertDescription>
          </Alert>

          <component :is="activeSectionComponent" />
        </div>
      </main>
    </SidebarInset>
  </SidebarProvider>
</template>

<style scoped>
.settings-scroll-area {
  scrollbar-color: color-mix(in oklch, var(--muted-foreground) 55%, transparent) transparent;
  scrollbar-width: thin;
}

.settings-scroll-area::-webkit-scrollbar {
  width: 10px;
}

.settings-scroll-area::-webkit-scrollbar-track {
  background: transparent;
}

.settings-scroll-area::-webkit-scrollbar-thumb {
  background: color-mix(in oklch, var(--muted-foreground) 45%, transparent);
  background-clip: padding-box;
  border: 2px solid transparent;
  border-radius: 999px;
}

.settings-scroll-area::-webkit-scrollbar-thumb:hover {
  background: color-mix(in oklch, var(--muted-foreground) 65%, transparent);
  background-clip: padding-box;
}
</style>
