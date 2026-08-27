<script setup lang="ts">
import { Volume1Icon, Volume2Icon, VolumeXIcon } from '@lucide/vue'
import { computed, onBeforeUnmount, onMounted, shallowRef } from 'vue'

import { Slider } from '@/components/ui/slider'
import {
  controlCurrentApplicationVolume,
  getCurrentApplicationVolume,
  hideApplicationVolumeFlyout,
  listenToVolumeFlyoutHidden,
  listenToVolumeFlyoutShown,
  readVolumeWheelDelta,
  reportApplicationVolumeState,
  reportVolumeFlyoutHover,
  showReadyApplicationVolumeFlyout,
  type ApplicationVolumeState,
} from '@/lib/application-volume-api'
import { waitForColorModeReady } from '@/lib/color-mode'
import { TauriListenerScope } from '@/lib/tauri-listener-scope'

const state = shallowRef<ApplicationVolumeState | null>(null)
const displayLevel = shallowRef(0)
const sessionKey = shallowRef<number>()
const accentColor = shallowRef('#0078D4')
const listenerScope = new TauriListenerScope()
const volumeIcon = computed(() => {
  if (!state.value || state.value.muted || displayLevel.value === 0) return VolumeXIcon
  return displayLevel.value < 50 ? Volume1Icon : Volume2Icon
})
let pollTimer: number | undefined
let setLevelTimer: number | undefined
let pendingLevel: number | undefined
let pendingAdjustment = 0
let isControlling = false
let wheelAccumulator = 0
let requestRevision = 0

function applyState(nextState: ApplicationVolumeState | null): void {
  state.value = nextState
  if (nextState) {
    displayLevel.value = nextState.levelPercent
    void reportApplicationVolumeState(nextState)
  }
}

async function refreshVolume(): Promise<void> {
  const expectedSessionKey = sessionKey.value
  if (expectedSessionKey === undefined || isControlling) return
  const revision = ++requestRevision
  try {
    const nextState = await getCurrentApplicationVolume(expectedSessionKey)
    if (revision === requestRevision && sessionKey.value === expectedSessionKey) {
      applyState(nextState)
      if (!nextState) void hideApplicationVolumeFlyout()
    }
  } catch {
    if (revision === requestRevision) void hideApplicationVolumeFlyout()
  }
}

function stopPolling(): void {
  if (pollTimer !== undefined) window.clearInterval(pollTimer)
  pollTimer = undefined
}

function startPolling(): void {
  stopPolling()
  pollTimer = window.setInterval(() => void refreshVolume(), 500)
}

async function drainPendingLevel(): Promise<void> {
  if (isControlling) return
  isControlling = true
  try {
    while (pendingLevel !== undefined && sessionKey.value !== undefined) {
      const levelPercent = pendingLevel
      pendingLevel = undefined
      const nextState = await controlCurrentApplicationVolume(sessionKey.value, {
        type: 'setLevel',
        levelPercent,
      })
      applyState(nextState)
    }
  } catch {
    void refreshVolume()
  } finally {
    isControlling = false
    if (pendingAdjustment !== 0) void drainPendingAdjustment()
  }
}

function queueLevel(levelPercent: number): void {
  displayLevel.value = Math.round(Math.max(0, Math.min(100, levelPercent)))
  pendingLevel = displayLevel.value
  if (setLevelTimer !== undefined) window.clearTimeout(setLevelTimer)
  setLevelTimer = window.setTimeout(() => {
    setLevelTimer = undefined
    void drainPendingLevel()
  }, 50)
}

function handleSliderChange(value: number[] | undefined): void {
  const level = value?.[0]
  if (level !== undefined) queueLevel(level)
}

async function drainPendingAdjustment(): Promise<void> {
  if (sessionKey.value === undefined || isControlling) return
  isControlling = true
  try {
    while (pendingAdjustment !== 0 && sessionKey.value !== undefined) {
      const deltaPercent = pendingAdjustment
      pendingAdjustment = 0
      applyState(
        await controlCurrentApplicationVolume(sessionKey.value, {
          type: 'adjust',
          deltaPercent,
        }),
      )
    }
  } catch {
    void refreshVolume()
  } finally {
    isControlling = false
    if (pendingLevel !== undefined) void drainPendingLevel()
  }
}

function queueAdjustment(deltaPercent: number): void {
  pendingAdjustment = Math.max(-20, Math.min(20, pendingAdjustment + deltaPercent))
  void drainPendingAdjustment()
}

function handleWheel(event: WheelEvent): void {
  const delta = readVolumeWheelDelta(event)
  if (delta === null || !state.value) return
  event.preventDefault()
  if (wheelAccumulator !== 0 && Math.sign(wheelAccumulator) !== Math.sign(delta)) {
    wheelAccumulator = 0
  }
  wheelAccumulator += delta
  if (Math.abs(wheelAccumulator) < 40) return
  queueAdjustment(wheelAccumulator < 0 ? 2 : -2)
  wheelAccumulator = 0
}

onMounted(async () => {
  const lifecycleRevision = listenerScope.activate()
  await Promise.all([
    listenerScope.register(
      lifecycleRevision,
      listenToVolumeFlyoutShown(({ sessionKey: nextSessionKey, accentColor: nextAccentColor }) => {
        requestRevision += 1
        sessionKey.value = nextSessionKey
        accentColor.value = nextAccentColor
        void refreshVolume()
        startPolling()
      }),
    ),
    listenerScope.register(
      lifecycleRevision,
      listenToVolumeFlyoutHidden(() => {
        stopPolling()
      }),
    ),
  ])
  await waitForColorModeReady()
  if (listenerScope.isCurrent(lifecycleRevision)) await showReadyApplicationVolumeFlyout()
})

onBeforeUnmount(() => {
  listenerScope.deactivate()
  stopPolling()
  if (setLevelTimer !== undefined) window.clearTimeout(setLevelTimer)
})
</script>

<template>
  <main
    class="flex h-screen w-screen items-center justify-center bg-transparent p-1"
    @mouseenter="reportVolumeFlyoutHover(true)"
    @mouseleave="reportVolumeFlyoutHover(false)"
  >
    <section
      class="bg-secondary text-secondary-foreground flex h-full w-full flex-col items-center gap-2 rounded-md border py-3 shadow-lg"
      aria-label="当前应用音量"
      :style="{ '--volume-accent': accentColor }"
    >
      <span class="text-xs font-semibold tabular-nums">{{ displayLevel }}%</span>
      <div class="flex h-24 w-6 items-center justify-center" @wheel="handleWheel">
        <Slider
          :model-value="[displayLevel]"
          :min="0"
          :max="100"
          :step="1"
          orientation="vertical"
          class="volume-slider h-full! min-h-0!"
          aria-label="当前应用音量"
          @update:model-value="handleSliderChange"
        />
      </div>
      <component :is="volumeIcon" class="size-4 shrink-0" aria-hidden="true" />
    </section>
  </main>
</template>

<style scoped>
.volume-slider :deep([data-slot='slider-track']) {
  width: 0.25rem;
}

.volume-slider :deep([data-slot='slider-range']) {
  background-color: var(--volume-accent);
}

.volume-slider :deep([data-slot='slider-thumb']) {
  width: 0.75rem;
  height: 0.75rem;
  border-color: var(--volume-accent);
}
</style>
