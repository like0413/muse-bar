import { invoke } from '@tauri-apps/api/core'

export interface BarWidthMeasurement {
  naturalWidth: number
  targetWidth: number
  minimumWidth: number
  maximumWidth: number
}

/** 将 Bar 内容的自然逻辑宽度报告给 Rust，并返回设置边界限制后的目标宽度。 */
export function reportBarContentWidth(naturalWidth: number): Promise<BarWidthMeasurement> {
  return invoke<BarWidthMeasurement>('report_bar_content_width', { naturalWidth })
}
