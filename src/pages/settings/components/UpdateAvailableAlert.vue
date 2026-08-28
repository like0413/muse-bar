<script setup lang="ts">
import { DownloadIcon, SparklesIcon } from '@lucide/vue'
import { storeToRefs } from 'pinia'

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Spinner } from '@/components/ui/spinner'

import { useUpdateStore } from '../update-store'

const updateStore = useUpdateStore()
const { status, showAvailablePrompt, progressPercent } = storeToRefs(updateStore)
</script>

<template>
  <Alert
    v-if="showAvailablePrompt || status?.stage === 'downloading' || status?.stage === 'installing'"
  >
    <SparklesIcon />
    <AlertTitle>
      {{
        status?.stage === 'available'
          ? `Muse Bar ${status.availableVersion} 可用`
          : '正在更新 Muse Bar'
      }}
    </AlertTitle>
    <AlertDescription class="flex flex-col gap-3">
      <p v-if="status?.stage === 'available'" class="whitespace-pre-line">
        {{ status.notes || '此版本没有提供更新说明。' }}
      </p>
      <template v-else-if="status?.stage === 'downloading'">
        <div class="flex items-center gap-2">
          <Spinner v-if="progressPercent === undefined" />
          <span
            >正在下载安装包{{ progressPercent === undefined ? '' : `：${progressPercent}%` }}</span
          >
        </div>
        <Progress v-if="progressPercent !== undefined" :model-value="progressPercent" />
      </template>
      <p v-else>安装程序即将启动，Muse Bar 会暂时退出并在完成后重新打开。</p>

      <div v-if="status?.stage === 'available'" class="flex flex-wrap gap-2">
        <Button size="sm" @click="updateStore.install">
          <DownloadIcon data-icon="inline-start" />
          立即更新
        </Button>
        <Button size="sm" variant="outline" @click="updateStore.dismiss">稍后</Button>
      </div>
    </AlertDescription>
  </Alert>
</template>
