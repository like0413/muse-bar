<script setup lang="ts">
import { readCurrentWindowLabel } from '@/lib/window-label'

const windowLabel = readCurrentWindowLabel()
</script>

<template>
  <main class="flex min-h-screen flex-col justify-center gap-1 p-4">
    <p class="text-muted-foreground text-sm">Taskbar surface</p>
    <h1 class="text-lg font-semibold">Muse Bar</h1>
    <p class="text-sm">
      Window label: <code>{{ windowLabel }}</code>
    </p>
  </main>
</template>
