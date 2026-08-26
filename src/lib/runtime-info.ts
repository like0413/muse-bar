import { invoke } from '@tauri-apps/api/core'

export interface RuntimeInfo {
  applicationVersion: string
  startedAtUnixMs: number
}

/** 调用 Rust 命令读取当前应用进程的运行信息。 */
export function getRuntimeInfo(): Promise<RuntimeInfo> {
  return invoke<RuntimeInfo>('get_runtime_info')
}
