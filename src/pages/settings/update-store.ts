import { defineStore } from 'pinia'
import { computed, shallowRef } from 'vue'

import { TauriListenerScope } from '@/lib/tauri-listener-scope'
import {
  checkForUpdate,
  getUpdateStatus,
  installUpdate,
  listenToUpdateStatus,
  type UpdateStatus,
} from '@/lib/updater-api'
import { getErrorMessage } from '@/lib/utils'

export const useUpdateStore = defineStore('updater', () => {
  const status = shallowRef<UpdateStatus>()
  const clientError = shallowRef('')
  const dismissedVersion = shallowRef('')
  const listenerScope = new TauriListenerScope()

  const isBusy = computed(
    () =>
      status.value?.stage === 'checking' ||
      status.value?.stage === 'downloading' ||
      status.value?.stage === 'installing',
  )
  const availableVersion = computed(() => status.value?.availableVersion)
  const showAvailablePrompt = computed(
    () =>
      status.value?.stage === 'available' &&
      Boolean(availableVersion.value) &&
      dismissedVersion.value !== availableVersion.value,
  )
  const progressPercent = computed(() => {
    const current = status.value
    if (!current?.totalBytes || current.totalBytes <= 0) return undefined
    return Math.min(100, Math.round((current.downloadedBytes / current.totalBytes) * 100))
  })

  /** 建立唯一的状态监听器，并补读监听前已产生的后台检查结果。 */
  async function start(): Promise<void> {
    if (listenerScope.isActive) return
    const lifecycleRevision = listenerScope.activate()
    try {
      await listenerScope.register(
        lifecycleRevision,
        listenToUpdateStatus((nextStatus) => {
          if (!listenerScope.isCurrent(lifecycleRevision)) return
          status.value = nextStatus
          clientError.value = ''
        }),
      )
      const initialStatus = await getUpdateStatus()
      if (listenerScope.isCurrent(lifecycleRevision)) status.value = initialStatus
    } catch (error) {
      if (listenerScope.isCurrent(lifecycleRevision)) clientError.value = getErrorMessage(error)
    }
  }

  /** 手动检查更新，IPC 失败时保留一条设置页可见的错误。 */
  async function check(): Promise<void> {
    if (isBusy.value) return
    clientError.value = ''
    try {
      status.value = await checkForUpdate()
    } catch (error) {
      clientError.value = getErrorMessage(error)
      await refreshStatusAfterFailure()
    }
  }

  /** 安装当前提示的版本；后端会在下载前再次验证版本是否仍然一致。 */
  async function install(): Promise<void> {
    const expectedVersion = availableVersion.value
    if (!expectedVersion || isBusy.value) return
    clientError.value = ''
    try {
      await installUpdate(expectedVersion)
    } catch (error) {
      clientError.value = getErrorMessage(error)
      await refreshStatusAfterFailure()
    }
  }

  async function refreshStatusAfterFailure(): Promise<void> {
    try {
      status.value = await getUpdateStatus()
    } catch {
      // 保留原始 IPC 错误，避免二次读取失败覆盖真正原因。
    }
  }

  /** 仅隐藏本次发现的版本，不写入用户设置。 */
  function dismiss(): void {
    dismissedVersion.value = availableVersion.value ?? ''
  }

  function stop(): void {
    listenerScope.deactivate()
  }

  return {
    status,
    clientError,
    isBusy,
    availableVersion,
    showAvailablePrompt,
    progressPercent,
    start,
    stop,
    check,
    install,
    dismiss,
  }
})
