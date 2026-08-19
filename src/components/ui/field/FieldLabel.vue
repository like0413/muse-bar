<script setup lang="ts">
import type { HTMLAttributes } from 'vue'

import { Label } from '@/components/ui/label'
import { cn } from '@/lib/utils'

const props = defineProps<{
  class?: HTMLAttributes['class']
}>()
</script>

<template>
  <Label
    data-slot="field-label"
    :class="
      cn(
        'group/field-label peer/field-label flex w-fit gap-2 leading-snug group-data-[disabled=true]/field:opacity-50',
        props.class,
      )
    "
  >
    <slot />
  </Label>
</template>
