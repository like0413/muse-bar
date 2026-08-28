import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type UpdateStage =
  | 'idle'
  | 'checking'
  | 'available'
  | 'downloading'
  | 'installing'
  | 'upToDate'
  | 'error'

export interface UpdateStatus {
  stage: UpdateStage
  currentVersion: string
  availableVersion?: string
  notes?: string
  publishedAt?: string
  downloadedBytes: number
  totalBytes?: number
  error?: string
}

const UPDATE_STATUS_EVENT = 'updater-status'

/** 读取由 Rust 后台更新器持有的状态快照。 */
export function getUpdateStatus(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>('get_update_status')
}

/** 显式检查 GitHub Release，并返回检查后的状态。 */
export function checkForUpdate(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>('check_for_update')
}

/** 下载、验签并安装用户确认的目标版本。 */
export function installUpdate(expectedVersion: string): Promise<void> {
  return invoke('install_update', { expectedVersion })
}

/** 监听后台检查与下载安装期间的状态变化。 */
export function listenToUpdateStatus(handler: (status: UpdateStatus) => void): Promise<UnlistenFn> {
  return listen<UpdateStatus>(UPDATE_STATUS_EVENT, (event) => handler(event.payload))
}
