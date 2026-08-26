const recordedMilestones = new Set<string>()

/** 在开发者工具中记录一次前端启动里程碑；同一 WebView 的同名节点只记录一次。 */
export function markStartupMilestone(name: string): void {
  if (recordedMilestones.has(name)) return
  recordedMilestones.add(name)
  const markName = `muse-bar:${name}`
  performance.mark(markName)
  console.info(`[startup] ${name}: +${performance.now().toFixed(1)} ms`)
}
