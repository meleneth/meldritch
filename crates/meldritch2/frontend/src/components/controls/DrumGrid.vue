<script setup lang="ts">
import { computed, ref } from "vue";
import type { DrumGridControl } from "../../types/protocol";
const props = defineProps<{ control: DrumGridControl }>();
const emit = defineEmits<{
  toggle: [string, number]; velocity: [string, number, number]; mute: [string, boolean];
}>();
const selected = ref<{ row: string; step: number } | null>(null);
const rowState = (id: string) => props.control.state_rows?.find((row) => row.id === id);
const hit = (row: string, step: number) => rowState(row)?.hits.find((hit) => hit.step === step);
const selectedHit = computed(() => selected.value && hit(selected.value.row, selected.value.step));
function press(row: string, step: number) {
  if (hit(row, step)) selected.value = { row, step };
  else { emit("toggle", row, step); selected.value = { row, step }; }
}
function velocity(value: string) {
  if (selected.value) emit("velocity", selected.value.row, selected.value.step, Number(value));
}
</script>
<template>
  <div class="drum-grid" :aria-label="control.label">
    <header><strong>{{ control.current_kit || control.kit }}</strong><small> · active kit</small></header>
    <div class="drum-header"><span></span><span v-for="step in control.step_count" :key="step">{{ String(step).padStart(2, "0") }}</span></div>
    <div v-for="row in control.rows" :key="row.id" class="drum-row">
      <button class="drum-label" :class="{ muted: rowState(row.id)?.muted }" :disabled="control.read_only" @click="$emit('mute', row.id, !rowState(row.id)?.muted)">{{ row.label }}<small>{{ rowState(row.id)?.muted ? "MUTED" : "MUTE" }}</small></button>
      <button v-for="step in control.step_count" :key="step" :disabled="control.read_only" :class="{ on: hit(row.id, step - 1), play: control.position === step - 1, selected: selected?.row === row.id && selected.step === step - 1 }" :aria-pressed="!!hit(row.id, step - 1)" @click="press(row.id, step - 1)" @dblclick="$emit('toggle', row.id, step - 1)"><span :style="{ opacity: hit(row.id, step - 1) ? Math.max(.3, hit(row.id, step - 1)!.velocity / 127) : 1 }"></span></button>
    </div>
    <label v-if="selectedHit">Velocity {{ selectedHit.velocity }}<input type="range" min="1" max="127" :value="selectedHit.velocity" :disabled="control.read_only" @change="velocity(($event.target as HTMLInputElement).value)" /></label>
  </div>
</template>
