import type { UnlistenFn } from '@tauri-apps/api/event'

/** 管理一次页面激活期间创建的 Tauri 监听器，并隔离上一次激活遗留的异步结果。 */
export class TauriListenerScope {
  private revision = 0
  private activeRevision: number | undefined
  private readonly listeners: UnlistenFn[] = []

  get isActive(): boolean {
    return this.activeRevision !== undefined
  }

  activate(): number {
    if (this.activeRevision !== undefined) return this.activeRevision
    this.activeRevision = ++this.revision
    return this.activeRevision
  }

  isCurrent(revision: number): boolean {
    return this.activeRevision === revision
  }

  async register(revision: number, listenerPromise: Promise<UnlistenFn>): Promise<void> {
    const stopListener = await listenerPromise
    if (!this.isCurrent(revision)) {
      stopListener()
      return
    }
    this.listeners.push(stopListener)
  }

  deactivate(): void {
    this.activeRevision = undefined
    this.revision += 1
    for (const stopListener of this.listeners.splice(0)) stopListener()
  }
}
