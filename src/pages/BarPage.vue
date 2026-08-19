<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import { onBeforeUnmount, onMounted, ref } from 'vue'

import {
  getCurrentMediaMetadata,
  getCurrentPlaybackStatus,
  listenToCurrentMediaMetadataChanges,
  listenToCurrentPlaybackStatusChanges,
  type CurrentMediaMetadata,
  type CurrentPlaybackStatus,
} from '@/lib/media-api'

const mediaMetadataStatus = ref('正在读取媒体信息')
const mediaMetadataDetails = ref('')
const playbackStatusText = ref('状态读取中')
let stopMediaMetadataListener: UnlistenFn | undefined
let stopPlaybackStatusListener: UnlistenFn | undefined
let hasUnmounted = false

const playbackStatusLabels: Record<CurrentPlaybackStatus, string> = {
  closed: '已关闭',
  opened: '已打开',
  changing: '切换中',
  stopped: '已停止',
  playing: '播放中',
  paused: '已暂停',
  unknown: '状态未知',
}

/** 将当前会话元数据转换为 Bar 的文本和完整悬停说明。 */
function showCurrentMediaMetadata(metadata: CurrentMediaMetadata | null) {
  if (!metadata) {
    mediaMetadataStatus.value = '当前没有媒体会话'
    mediaMetadataDetails.value = mediaMetadataStatus.value
    return
  }

  const title = metadata.title || '未知标题'
  mediaMetadataStatus.value = metadata.artist ? `${title} · ${metadata.artist}` : title
  mediaMetadataDetails.value = `${metadata.sourceAppId}\n标题：${title}\n歌手：${metadata.artist || '未知歌手'}`
}

/** 将 Windows 播放状态转换为当前验证页面使用的中文文本。 */
function showCurrentPlaybackStatus(status: CurrentPlaybackStatus | null) {
  playbackStatusText.value = status ? playbackStatusLabels[status] : '无播放状态'
}

/** 从 Rust 主动读取一次 Windows 当前会话元数据。 */
async function loadCurrentMediaMetadata() {
  try {
    showCurrentMediaMetadata(await getCurrentMediaMetadata())
  } catch {
    mediaMetadataStatus.value = '媒体信息读取失败'
    mediaMetadataDetails.value = mediaMetadataStatus.value
  }
}

/** 从 Rust 主动读取一次 Windows 当前会话播放状态。 */
async function loadCurrentPlaybackStatus() {
  try {
    showCurrentPlaybackStatus(await getCurrentPlaybackStatus())
  } catch {
    playbackStatusText.value = '状态读取失败'
  }
}

/** 先建立元数据事件订阅，再读取当前值，避免页面初始化期间遗漏切歌。 */
async function startMediaMetadataListener() {
  try {
    const stopListener = await listenToCurrentMediaMetadataChanges(showCurrentMediaMetadata)
    if (hasUnmounted) {
      stopListener()
      return
    }

    stopMediaMetadataListener = stopListener
    await loadCurrentMediaMetadata()
  } catch {
    mediaMetadataStatus.value = '媒体信息监听失败'
    mediaMetadataDetails.value = mediaMetadataStatus.value
  }
}

/** 先建立播放状态订阅，再读取当前状态，避免页面初始化期间遗漏变化。 */
async function startPlaybackStatusListener() {
  try {
    const stopListener = await listenToCurrentPlaybackStatusChanges(showCurrentPlaybackStatus)
    if (hasUnmounted) {
      stopListener()
      return
    }

    stopPlaybackStatusListener = stopListener
    await loadCurrentPlaybackStatus()
  } catch {
    playbackStatusText.value = '状态监听失败'
  }
}

onMounted(startMediaMetadataListener)
onMounted(startPlaybackStatusListener)

onBeforeUnmount(() => {
  hasUnmounted = true
  stopMediaMetadataListener?.()
  stopPlaybackStatusListener?.()
})
</script>

<template>
  <main class="flex h-screen w-screen items-center justify-center bg-transparent p-1">
    <section
      aria-label="Muse Bar"
      class="bg-secondary text-secondary-foreground flex h-full w-full items-center justify-center rounded-md border px-3 text-sm font-medium"
    >
      <span class="truncate" :title="mediaMetadataDetails">
        {{ playbackStatusText }} · {{ mediaMetadataStatus }}
      </span>
    </section>
  </main>
</template>
