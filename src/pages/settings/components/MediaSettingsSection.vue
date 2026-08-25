<script setup lang="ts">
import { AlertCircleIcon, Music2Icon } from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed } from 'vue'

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import type { MediaActivityReason, MediaPlayerKind } from '@/lib/media-api'

import { useSettingsStore } from '../settings-store'

const settingsStore = useSettingsStore()
const { mediaSnapshot, mediaSnapshotError, mediaSessionIdentities, mediaSessionActivities } =
  storeToRefs(settingsStore)

const playerKindLabels: Record<MediaPlayerKind, string> = {
  qqMusic: 'QQ 音乐',
  neteaseCloudMusic: '网易云音乐',
  kugouMusic: '酷狗音乐',
  qishuiMusic: '汽水音乐',
  other: '普通系统媒体',
}
const activityReasonLabels: Record<MediaActivityReason, string> = {
  detectedPlaying: '启动时已在播放',
  playbackStarted: '开始播放',
  trackChanged: '切换歌曲',
  becameCurrent: '成为系统当前会话',
}
const currentCapabilities = computed(() => {
  const capabilities = mediaSnapshot.value?.capabilities
  if (!capabilities) return []
  return [
    { label: '播放', supported: capabilities.canPlay },
    { label: '暂停', supported: capabilities.canPause },
    { label: '上一曲', supported: capabilities.canPrevious },
    { label: '下一曲', supported: capabilities.canNext },
    { label: '跳转', supported: capabilities.canSeek },
  ]
})

/** 将布尔控制能力转换为诊断页统一使用的中文状态。 */
function formatCapability(supported: boolean): string {
  return supported ? '支持' : '不支持'
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <Alert v-if="mediaSnapshotError" variant="destructive">
      <AlertCircleIcon />
      <AlertTitle>媒体状态读取失败</AlertTitle>
      <AlertDescription>{{ mediaSnapshotError }}</AlertDescription>
    </Alert>

    <Card>
      <CardHeader>
        <CardTitle>当前展示媒体</CardTitle>
        <CardDescription>Bar 当前选择的系统媒体会话。</CardDescription>
      </CardHeader>
      <CardContent>
        <div v-if="mediaSnapshot" class="flex items-center gap-4">
          <Avatar class="size-16 rounded-lg">
            <AvatarImage
              v-if="mediaSnapshot.artworkDataUrl"
              :src="mediaSnapshot.artworkDataUrl"
              :alt="mediaSnapshot.title || '当前歌曲封面'"
            />
            <AvatarFallback class="rounded-lg"><Music2Icon /></AvatarFallback>
          </Avatar>
          <div class="min-w-0 flex-1">
            <p class="truncate font-semibold">{{ mediaSnapshot.title || '未知歌曲' }}</p>
            <p class="text-muted-foreground truncate text-sm">
              {{ mediaSnapshot.artist || '未知歌手' }}
            </p>
            <div class="mt-2 flex flex-wrap gap-2">
              <Badge>{{ playerKindLabels[mediaSnapshot.playerKind] }}</Badge>
              <Badge variant="outline">{{ mediaSnapshot.playbackStatus }}</Badge>
              <Badge variant="secondary">当前选择项</Badge>
            </div>
          </div>
        </div>
        <p v-else class="text-muted-foreground text-sm">当前没有可展示的系统媒体会话。</p>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>控制能力</CardTitle>
        <CardDescription>是否可用由播放器通过 Windows SMTC 声明。</CardDescription>
      </CardHeader>
      <CardContent>
        <dl v-if="mediaSnapshot" class="grid grid-cols-2 gap-3 sm:grid-cols-5">
          <div
            v-for="capability in currentCapabilities"
            :key="capability.label"
            class="bg-muted rounded-lg border p-3 text-center"
          >
            <dt class="text-sm font-medium">{{ capability.label }}</dt>
            <dd class="text-muted-foreground mt-1 text-xs">
              {{ formatCapability(capability.supported) }}
            </dd>
          </div>
        </dl>
        <p v-else class="text-muted-foreground text-sm">选择媒体会话后显示控制能力。</p>
        <p class="text-muted-foreground mt-4 text-sm">
          不支持的能力会使 Bar 上对应按钮禁用，这通常表示播放器没有向 Windows 暴露该操作。
        </p>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>检测到的会话</CardTitle>
        <CardDescription>共 {{ mediaSessionIdentities.length }} 个系统媒体会话。</CardDescription>
      </CardHeader>
      <CardContent class="flex flex-col gap-2">
        <p v-if="mediaSessionIdentities.length === 0" class="text-muted-foreground text-sm">
          暂无媒体会话。
        </p>
        <template v-else>
          <div
            v-for="identity in mediaSessionIdentities"
            :key="identity.sessionKey"
            class="flex items-center justify-between gap-3 rounded-lg border p-3"
          >
            <div class="min-w-0">
              <p class="font-medium">{{ playerKindLabels[identity.playerKind] }}</p>
              <code class="text-muted-foreground block truncate text-xs">{{
                identity.sourceAppId
              }}</code>
            </div>
            <Badge v-if="identity.sessionKey === mediaSnapshot?.sessionKey">正在显示</Badge>
          </div>
        </template>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>最近活动</CardTitle>
        <CardDescription>用于解释多个播放器同时存在时的选择顺序。</CardDescription>
      </CardHeader>
      <CardContent class="flex flex-col gap-2">
        <p v-if="mediaSessionActivities.length === 0" class="text-muted-foreground text-sm">
          暂无有效活动记录。
        </p>
        <template v-else>
          <div
            v-for="activity in mediaSessionActivities"
            :key="activity.sessionKey"
            class="rounded-lg border p-3"
          >
            <div class="flex items-center justify-between gap-3">
              <p class="truncate font-medium">
                {{ activity.title || playerKindLabels[activity.playerKind] }}
              </p>
              <Badge variant="outline">
                {{
                  activity.lastActivityReason
                    ? activityReasonLabels[activity.lastActivityReason]
                    : '尚未活动'
                }}
              </Badge>
            </div>
            <p class="text-muted-foreground mt-1 truncate text-xs">
              {{ activity.artist || activity.sourceAppId }}
            </p>
          </div>
        </template>
      </CardContent>
    </Card>
  </div>
</template>
