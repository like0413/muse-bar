import {
  ActivityIcon,
  Music2Icon,
  PaletteIcon,
  PanelTopIcon,
  Settings2Icon,
  type LucideIcon,
} from '@lucide/vue'

export type SettingsSection = 'taskbar' | 'appearance' | 'media' | 'general' | 'diagnostics'

export interface SettingsNavigationItem {
  id: SettingsSection
  label: string
  description: string
  icon: LucideIcon
}

export const SETTINGS_NAVIGATION: ReadonlyArray<SettingsNavigationItem> = [
  {
    id: 'taskbar',
    label: '任务栏',
    description: '调整显示器、位置、宽度和歌词模式。',
    icon: PanelTopIcon,
  },
  {
    id: 'appearance',
    label: '外观',
    description: '调整颜色模式、元素排列、进度和滚动文本。',
    icon: PaletteIcon,
  },
  {
    id: 'media',
    label: '媒体',
    description: '查看当前媒体、控制能力和播放器活动。',
    icon: Music2Icon,
  },
  {
    id: 'general',
    label: '常规',
    description: '管理开机启动与后台运行行为。',
    icon: Settings2Icon,
  },
  {
    id: 'diagnostics',
    label: '诊断与关于',
    description: '查看运行环境、任务栏状态和最近错误。',
    icon: ActivityIcon,
  },
]

/** 按设置分区标识查找导航信息，未知值回退到任务栏分区。 */
export function getSettingsNavigationItem(section: SettingsSection): SettingsNavigationItem {
  return SETTINGS_NAVIGATION.find((item) => item.id === section) ?? SETTINGS_NAVIGATION[0]!
}
