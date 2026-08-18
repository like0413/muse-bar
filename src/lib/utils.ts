import type { ClassValue } from 'clsx'
import { clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

/** 合并条件类名，并消除互相冲突的 Tailwind 类。 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
