import type { SettingsPatch, SettingsPayload } from '@/lib/settings-api'

export interface AppearanceSettingsCardProps {
  settings: SettingsPayload | undefined
  disabled: boolean
}

export interface AppearanceSettingsCardEmits {
  change: [patch: SettingsPatch]
}
