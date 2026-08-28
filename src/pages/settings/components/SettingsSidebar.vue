<script setup lang="ts">
import { Music2Icon } from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed } from 'vue'

import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/components/ui/sidebar'

import { SETTINGS_NAVIGATION, type SettingsSection } from '../settings-navigation'
import { useSettingsStore } from '../settings-store'

const activeSection = defineModel<SettingsSection>({ required: true })
const settingsStore = useSettingsStore()
const { runtimeInfo } = storeToRefs(settingsStore)

const versionLabel = computed(() =>
  runtimeInfo.value ? `版本 ${runtimeInfo.value.applicationVersion}` : 'Windows 11 媒体工具',
)
</script>

<template>
  <Sidebar collapsible="icon" class="group-data-[side=left]:border-r-0">
    <SidebarHeader>
      <SidebarMenu>
        <SidebarMenuItem>
          <SidebarMenuButton size="lg" tooltip="Muse Bar 设置">
            <div
              class="bg-sidebar-primary text-sidebar-primary-foreground flex aspect-square size-8 items-center justify-center rounded-lg"
            >
              <Music2Icon />
            </div>
            <div class="grid flex-1 text-left text-sm leading-tight">
              <span class="truncate font-semibold">Muse Bar</span>
              <span class="truncate text-xs">任务栏媒体工具</span>
            </div>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarHeader>

    <SidebarContent>
      <SidebarGroup>
        <SidebarGroupLabel>设置</SidebarGroupLabel>
        <SidebarGroupContent>
          <SidebarMenu>
            <SidebarMenuItem v-for="item in SETTINGS_NAVIGATION" :key="item.id">
              <SidebarMenuButton
                :is-active="activeSection === item.id"
                :tooltip="item.label"
                @click="activeSection = item.id"
              >
                <component :is="item.icon" />
                <span>{{ item.label }}</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroupContent>
      </SidebarGroup>
    </SidebarContent>

    <SidebarFooter>
      <SidebarMenu>
        <SidebarMenuItem>
          <SidebarMenuButton size="lg" tooltip="Muse Bar 版本信息">
            <Avatar class="size-8 rounded-lg">
              <AvatarFallback class="rounded-lg">MB</AvatarFallback>
            </Avatar>
            <div class="grid flex-1 text-left text-sm leading-tight">
              <span class="truncate font-medium">Muse Bar</span>
              <span class="text-muted-foreground truncate text-xs">{{ versionLabel }}</span>
            </div>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarFooter>
  </Sidebar>
</template>
