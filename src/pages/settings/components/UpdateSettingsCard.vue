<script setup lang="ts">
import { AlertCircleIcon, DownloadIcon, RefreshCwIcon } from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed } from 'vue'

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'

import { useUpdateStore } from '../update-store'

const updateStore = useUpdateStore()
const { status, clientError, isBusy } = storeToRefs(updateStore)

const statusLabel = computed(() => {
  switch (status.value?.stage) {
    case 'checking':
      return '正在检查'
    case 'available':
      return `发现 ${status.value.availableVersion}`
    case 'downloading':
      return '正在下载'
    case 'installing':
      return '正在安装'
    case 'upToDate':
      return '已是最新版'
    case 'error':
      return '检查失败'
    default:
      return '尚未检查'
  }
})
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle>软件更新</CardTitle>
      <CardDescription>启动时自动检查 GitHub Release，也可以在这里手动检查。</CardDescription>
    </CardHeader>
    <CardContent class="flex flex-col gap-4">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div class="flex items-center gap-2 text-sm">
          <span class="text-muted-foreground">当前版本</span>
          <Badge variant="outline">{{ status?.currentVersion || '读取中' }}</Badge>
          <span class="text-muted-foreground">{{ statusLabel }}</span>
        </div>
        <div class="flex gap-2">
          <Button
            v-if="status?.stage === 'available'"
            size="sm"
            :disabled="isBusy"
            @click="updateStore.install"
          >
            <DownloadIcon data-icon="inline-start" />
            立即更新
          </Button>
          <Button size="sm" variant="outline" :disabled="isBusy" @click="updateStore.check">
            <RefreshCwIcon
              data-icon="inline-start"
              :class="{ 'animate-spin': status?.stage === 'checking' }"
            />
            检查更新
          </Button>
        </div>
      </div>

      <Alert v-if="clientError || status?.error" variant="destructive">
        <AlertCircleIcon />
        <AlertTitle>更新不可用</AlertTitle>
        <AlertDescription>{{ clientError || status?.error }}</AlertDescription>
      </Alert>
    </CardContent>
  </Card>
</template>
