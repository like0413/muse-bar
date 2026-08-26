<script setup lang="ts">
import { AlertCircleIcon, FolderOpenIcon, InfoIcon, RefreshCwIcon } from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed } from 'vue'

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'

import { useSettingsStore } from '../settings-store'

const settingsStore = useSettingsStore()
const {
  runtimeInfo,
  runtimeError,
  windowsVersion,
  settingsError,
  taskbarIdentity,
  taskbarDpi,
  taskbarOccupancy,
  taskbarDiagnosticError,
  taskbarMonitorError,
  mediaSnapshotError,
  logDirectoryError,
  isRefreshingDiagnostics,
  isOpeningLogDirectory,
} = storeToRefs(settingsStore)

const taskbarOccupancySourceLabels = {
  uiAutomation: 'UI Automation',
  win32Fallback: 'Win32 子窗口回退',
} as const
const recentErrors = computed(() =>
  [
    runtimeError.value,
    settingsError.value,
    taskbarDiagnosticError.value,
    taskbarMonitorError.value,
    mediaSnapshotError.value,
  ].filter(Boolean),
)

/** 将 Unix 毫秒时间戳转换为本地时间。 */
function formatStartedAt(startedAtUnixMs: number): string {
  return new Date(startedAtUnixMs).toLocaleString()
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex justify-end gap-2">
      <Button
        variant="outline"
        size="sm"
        :disabled="isRefreshingDiagnostics"
        :aria-describedby="taskbarDiagnosticError ? 'taskbar-diagnostic-error' : undefined"
        @click="settingsStore.loadTaskbarDiagnostics"
      >
        <RefreshCwIcon
          data-icon="inline-start"
          :class="{ 'animate-spin': isRefreshingDiagnostics }"
        />
        {{ isRefreshingDiagnostics ? '正在刷新' : '刷新诊断' }}
      </Button>
      <Button
        size="sm"
        :disabled="isOpeningLogDirectory"
        :aria-describedby="logDirectoryError ? 'log-directory-error' : undefined"
        @click="settingsStore.openLogs"
      >
        <FolderOpenIcon data-icon="inline-start" />
        {{ isOpeningLogDirectory ? '正在打开' : '打开日志目录' }}
      </Button>
    </div>

    <Alert v-if="logDirectoryError" id="log-directory-error" variant="destructive">
      <AlertCircleIcon />
      <AlertTitle>无法打开日志目录</AlertTitle>
      <AlertDescription>{{ logDirectoryError }}</AlertDescription>
    </Alert>

    <Card>
      <CardHeader>
        <CardTitle>运行环境</CardTitle>
        <CardDescription>Muse Bar 当前进程与窗口信息。</CardDescription>
      </CardHeader>
      <CardContent>
        <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-3 text-sm">
          <dt class="text-muted-foreground">Windows</dt>
          <dd>
            {{
              windowsVersion
                ? `${windowsVersion.productName} · ${windowsVersion.version}（Build ${windowsVersion.build}）`
                : '读取中'
            }}
          </dd>
          <dt class="text-muted-foreground">应用版本</dt>
          <dd>{{ runtimeInfo?.applicationVersion || '读取中' }}</dd>
          <dt class="text-muted-foreground">启动时间</dt>
          <dd>{{ runtimeInfo ? formatStartedAt(runtimeInfo.startedAtUnixMs) : '读取中' }}</dd>
          <dt class="text-muted-foreground">窗口标签</dt>
          <dd>
            <code>{{ settingsStore.windowLabel }}</code>
          </dd>
          <dt class="text-muted-foreground">实际宿主</dt>
          <dd><Badge>Child</Badge></dd>
        </dl>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>任务栏状态</CardTitle>
        <CardDescription>任务栏身份、DPI 和原生控件测量结果。</CardDescription>
      </CardHeader>
      <CardContent class="flex flex-col gap-4">
        <Alert v-if="taskbarDiagnosticError" id="taskbar-diagnostic-error" variant="destructive">
          <AlertCircleIcon />
          <AlertTitle>任务栏诊断失败</AlertTitle>
          <AlertDescription>{{ taskbarDiagnosticError }}</AlertDescription>
        </Alert>
        <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-3 text-sm">
          <dt class="text-muted-foreground">任务栏句柄</dt>
          <dd>
            <code>{{
              taskbarIdentity ? `0x${taskbarIdentity.hwnd.toString(16).toUpperCase()}` : '读取中'
            }}</code>
          </dd>
          <dt class="text-muted-foreground">Explorer PID</dt>
          <dd>{{ taskbarIdentity?.explorerProcessId ?? '读取中' }}</dd>
          <dt class="text-muted-foreground">DPI</dt>
          <dd>
            {{
              taskbarDpi
                ? `${taskbarDpi.dpi}（${Math.round(taskbarDpi.scaleFactor * 100)}%）`
                : '读取中'
            }}
          </dd>
          <dt class="text-muted-foreground">物理尺寸</dt>
          <dd>
            {{
              taskbarDpi
                ? `${taskbarDpi.physicalWidth} × ${taskbarDpi.physicalHeight} px`
                : '读取中'
            }}
          </dd>
          <dt class="text-muted-foreground">占用区来源</dt>
          <dd>
            {{
              taskbarOccupancy ? taskbarOccupancySourceLabels[taskbarOccupancy.source] : '读取中'
            }}
          </dd>
          <dt class="text-muted-foreground">占用区数量</dt>
          <dd>{{ taskbarOccupancy?.regionCount ?? '读取中' }}</dd>
          <dt class="text-muted-foreground">碰撞状态</dt>
          <dd>未启用（按阶段 9.5 的简化定位规则）</dd>
        </dl>
        <Alert v-if="taskbarOccupancy?.fallbackReason">
          <InfoIcon />
          <AlertTitle>已使用 Win32 回退</AlertTitle>
          <AlertDescription>{{ taskbarOccupancy.fallbackReason }}</AlertDescription>
        </Alert>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>最近错误</CardTitle>
        <CardDescription>本次设置窗口运行期间捕获的最近错误。</CardDescription>
      </CardHeader>
      <CardContent>
        <p v-if="recentErrors.length === 0" class="text-muted-foreground text-sm">
          当前没有记录到错误。
        </p>
        <ul v-else class="flex flex-col gap-2">
          <li
            v-for="error in recentErrors"
            :key="error"
            class="text-destructive rounded-lg border p-3 text-sm"
          >
            {{ error }}
          </li>
        </ul>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>关于 Muse Bar</CardTitle>
        <CardDescription>Windows 11 任务栏系统媒体工具。</CardDescription>
      </CardHeader>
      <CardContent class="text-muted-foreground flex flex-col gap-2 text-sm">
        <p>当前版本聚焦 Child 真嵌入和 Windows SMTC 媒体会话。</p>
        <p>真实歌词、Owner 回退、多屏同步和自动更新仍属于后续范围。</p>
      </CardContent>
    </Card>
  </div>
</template>
