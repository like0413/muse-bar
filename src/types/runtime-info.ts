/** Rust 返回给前端的应用运行信息。 */
export interface RuntimeInfo {
  applicationVersion: string
  startedAtUnixMs: number
}
