<script setup lang="ts">
import type { PresetControl } from "../../types/protocol";
defineProps<{ control: PresetControl }>();
defineEmits<{ select: [number] }>();
</script>

<template>
  <fieldset class="preset-selector" :disabled="control.read_only">
    <legend>{{ control.label }}</legend>
    <button
      v-for="(preset, index) in control.presets"
      :key="preset"
      type="button"
      :class="{ active: index === control.active, pending: index === control.pending }"
      :aria-pressed="index === control.active"
      @click="$emit('select', index)"
    >
      {{ preset }}
    </button>
    <small v-if="control.failed" role="alert">{{ control.failed }}</small>
  </fieldset>
</template>

<style scoped>
.preset-selector { display: flex; flex-wrap: wrap; gap: .5rem; border: 0; padding: 0; }
legend { width: 100%; margin-bottom: .35rem; font-weight: 700; }
button { min-height: 2.75rem; padding: .55rem .8rem; border: 1px solid #687386; border-radius: .5rem; background: #17202e; color: inherit; }
button.active { border-color: #66e3ff; background: #16475b; }
button.pending { outline: 2px dashed #ffd166; }
fieldset:disabled button { opacity: .55; }
small { width: 100%; color: #ff7b7b; }
</style>
