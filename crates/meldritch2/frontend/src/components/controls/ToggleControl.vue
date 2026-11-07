<script setup lang="ts">
import type { ToggleControl } from "../../types/protocol";

defineProps<{ control: ToggleControl }>();
defineEmits<{ change: [boolean] }>();
</script>

<template>
  <button
    class="toggle-control"
    type="button"
    :class="{ on: control.value }"
    :aria-pressed="control.value"
    :disabled="control.read_only"
    @click="$emit('change', !control.value)"
  >
    <span>{{ control.value ? "MUTED" : "LIVE" }}</span>
    {{ control.label }}
  </button>
</template>
