<script setup lang="ts">
import { computed, ref } from "vue";
import type { NoteEvent, PianoRollControl } from "../../types/protocol";
const props = defineProps<{ control: PianoRollControl }>();
const emit = defineEmits<{
  create: [Omit<NoteEvent, "id">];
  update: [NoteEvent];
  delete: [number];
}>();
const selected = ref<number | null>(null);
const pitches = computed(() => {
  const values: number[] = [];
  for (
    let pitch = props.control.pitch_max;
    pitch >= props.control.pitch_min;
    pitch--
  )
    values.push(pitch);
  return values;
});
const current = computed(() =>
  props.control.notes.find((note) => note.id === selected.value),
);
const noteAt = (pitch: number, step: number) =>
  props.control.notes.find(
    (note) => note.pitch === pitch && note.step === step,
  );
const noteName = (pitch: number) =>
  `${["C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B"][pitch % 12]}${Math.floor(pitch / 12) - 1}`;
function cell(pitch: number, step: number, event: MouseEvent) {
  const note = noteAt(pitch, step);
  if (note && event.shiftKey) emit("delete", note.id);
  else if (note) selected.value = note.id;
  else
    emit("create", { step, pitch, velocity: 100, duration: 1, legato: false });
}
function update(field: "pitch" | "velocity" | "duration", event: Event) {
  if (!current.value) return;
  emit("update", {
    ...current.value,
    [field]: Number((event.target as HTMLInputElement).value),
  });
}
</script>
<template>
  <div class="piano-roll">
    <header>
      <strong>{{ control.label }}</strong
      ><small>{{ control.voices }} voices</small>
    </header>
    <div class="piano-grid" :style="{ '--steps': control.step_count }">
      <template v-for="pitch in pitches" :key="pitch">
        <span class="pitch-label">{{ noteName(pitch) }}</span>
        <button
          v-for="step in control.step_count"
          :key="`${pitch}-${step}`"
          :class="{
            note: noteAt(pitch, step - 1),
            selected: noteAt(pitch, step - 1)?.id === selected,
            play: control.position === step - 1,
          }"
          :aria-label="`${noteName(pitch)} step ${step}`"
          :disabled="control.read_only"
          @click="cell(pitch, step - 1, $event)"
        >
          <span
            v-if="noteAt(pitch, step - 1)"
            :style="{
              width: `${Math.min(noteAt(pitch, step - 1)!.duration, control.step_count - step + 1) * 100}%`,
            }"
          />
        </button>
      </template>
    </div>
    <fieldset v-if="current" :disabled="control.read_only">
      <label
        >Pitch
        <input
          type="number"
          :min="control.pitch_min"
          :max="control.pitch_max"
          :value="current.pitch"
          @change="update('pitch', $event)"
      /></label>
      <label
        >Velocity
        <input
          type="number"
          min="1"
          max="127"
          :value="current.velocity"
          @change="update('velocity', $event)"
      /></label>
      <label
        >Duration
        <input
          type="number"
          min="1"
          :max="control.step_count - current.step"
          :value="current.duration"
          @change="update('duration', $event)"
      /></label>
      <button
        type="button"
        @click="
          emit('delete', current.id);
          selected = null;
        "
      >
        Delete note
      </button>
    </fieldset>
    <small>Tap a cell to add/select; Shift-tap or Delete removes.</small>
  </div>
</template>
