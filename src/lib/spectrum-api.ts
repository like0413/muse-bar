import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export interface SpectrumFrame {
  sessionKey: number
  levels: number[]
}

const SPECTRUM_FRAME_EVENT = 'spectrum-frame'
const BAR_VISIBILITY_CHANGED_EVENT = 'bar-visibility-changed'

export function startApplicationSpectrum(
  expectedSessionKey: number,
  frameRate: 20 | 30,
): Promise<void> {
  return invoke('start_application_spectrum', { expectedSessionKey, frameRate })
}

export function stopApplicationSpectrum(expectedSessionKey: number): Promise<void> {
  return invoke('stop_application_spectrum', { expectedSessionKey })
}

export function listenToSpectrumFrames(
  handleFrame: (frame: SpectrumFrame) => void,
): Promise<UnlistenFn> {
  return listen<SpectrumFrame>(SPECTRUM_FRAME_EVENT, (event) => handleFrame(event.payload))
}

export function listenToBarVisibilityChanges(
  handleVisibility: (visible: boolean) => void,
): Promise<UnlistenFn> {
  return listen<boolean>(BAR_VISIBILITY_CHANGED_EVENT, (event) => handleVisibility(event.payload))
}
