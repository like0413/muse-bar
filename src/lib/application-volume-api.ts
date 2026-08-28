import { invoke } from '@tauri-apps/api/core'
import { emitTo, listen, type UnlistenFn } from '@tauri-apps/api/event'

export interface ApplicationVolumeState {
  sessionKey: number
  levelPercent: number
  muted: boolean
}

export type ApplicationVolumeAction =
  | { type: 'setLevel'; levelPercent: number }
  | { type: 'adjust'; deltaPercent: number }
  | { type: 'toggleMute' }

export interface VolumeFlyoutAnchor {
  x: number
  y: number
  width: number
  height: number
}

interface VolumeFlyoutShownPayload {
  sessionKey: number
  accentColor: string
}

export function getCurrentApplicationVolume(
  expectedSessionKey: number,
): Promise<ApplicationVolumeState | null> {
  return invoke<ApplicationVolumeState | null>('get_current_application_volume', {
    expectedSessionKey,
  })
}

export function controlCurrentApplicationVolume(
  expectedSessionKey: number,
  action: ApplicationVolumeAction,
): Promise<ApplicationVolumeState> {
  return invoke<ApplicationVolumeState>('control_current_application_volume', {
    expectedSessionKey,
    action,
  })
}

export function showApplicationVolumeFlyout(
  anchor: VolumeFlyoutAnchor,
  expectedSessionKey: number,
  accentColor: string,
): Promise<void> {
  return invoke('show_application_volume_flyout', { anchor, expectedSessionKey, accentColor })
}

export function showReadyApplicationVolumeFlyout(): Promise<void> {
  return invoke('show_ready_application_volume_flyout')
}

export function hideApplicationVolumeFlyout(): Promise<void> {
  return invoke('hide_application_volume_flyout')
}

export function listenToVolumeFlyoutShown(
  handler: (payload: VolumeFlyoutShownPayload) => void,
): Promise<UnlistenFn> {
  return listen<VolumeFlyoutShownPayload>('application-volume-flyout-shown', ({ payload }) =>
    handler(payload),
  )
}

export function listenToVolumeFlyoutHidden(handler: () => void): Promise<UnlistenFn> {
  return listen('application-volume-flyout-hidden', handler)
}

export function listenToVolumeFlyoutHoverChanged(
  handler: (hovered: boolean) => void,
): Promise<UnlistenFn> {
  return listen<{ hovered: boolean }>('application-volume-flyout-hover-changed', ({ payload }) =>
    handler(payload.hovered),
  )
}

export function listenToApplicationVolumeStateChanged(
  handler: (state: ApplicationVolumeState) => void,
): Promise<UnlistenFn> {
  return listen<ApplicationVolumeState>('application-volume-state-changed', ({ payload }) =>
    handler(payload),
  )
}

export function reportVolumeFlyoutHover(hovered: boolean): Promise<void> {
  return emitTo('bar', 'application-volume-flyout-hover-changed', { hovered })
}

export function reportApplicationVolumeState(state: ApplicationVolumeState): Promise<void> {
  return emitTo('bar', 'application-volume-state-changed', state)
}

/** 将纵向滚轮增量标准化为像素；横向手势和 Ctrl 缩放不属于音量操作。 */
export function readVolumeWheelDelta(event: WheelEvent): number | null {
  if (event.ctrlKey || event.deltaY === 0 || Math.abs(event.deltaX) >= Math.abs(event.deltaY)) {
    return null
  }
  if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) return event.deltaY * 16
  if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) return event.deltaY * window.innerHeight
  return event.deltaY
}
