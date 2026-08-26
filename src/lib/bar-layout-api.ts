import { invoke } from '@tauri-apps/api/core'

export interface BarWidthMeasurement {
  naturalWidth: number
  targetWidth: number
  maximumWidth: number
  mode: 'content' | 'availableArea'
  applied: boolean
}

/** 将当前内容宽度策略报告给 Rust，并返回原生 Bar 实际采用的目标宽度。 */
export function reportBarContentWidth(
  naturalWidth: number,
  reduceMotion: boolean,
): Promise<BarWidthMeasurement> {
  return invoke<BarWidthMeasurement>('report_bar_content_width', { naturalWidth, reduceMotion })
}
