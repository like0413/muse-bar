<script setup lang="ts">
import { readCurrentWindowLabel } from '@/lib/window-label'

const windowLabel = readCurrentWindowLabel()
</script>

<template>
  <main class="flex min-h-screen flex-col justify-center gap-1 p-6">
    <p class="text-muted-foreground text-sm">Configuration surface</p>
    <h1 class="text-2xl font-semibold">Muse Bar Settings</h1>
    <p class="text-sm">
      Window label: <code>{{ windowLabel }}</code>
    </p>
  </main>
</template>
