<script setup lang="ts">
import type {
  AdsrParameter,
  NoteEvent,
  PatternControl,
  Plane,
} from "../types/protocol";
import ControlRenderer from "./ControlRenderer.vue";
defineProps<{ plane: Plane }>();
defineEmits<{
  set: [string, number];
  toggle: [string, number];
  fire: [string];
  noteCreate: [string, number, Omit<NoteEvent, "id">];
  noteUpdate: [string, number, NoteEvent];
  noteDelete: [string, number, number];
  pattern: [string, number, PatternControl["boundary"]];
  adsr: [string, AdsrParameter["name"], number];
}>();
</script>
<template>
  <section class="plane">
    <header>
      <h2>{{ plane.label }}</h2>
      <small>{{ plane.id }}</small>
    </header>
    <div class="plane-grid">
      <article v-for="control in plane.controls" :key="control.id">
        <ControlRenderer
          :control="control"
          @set="(id, value) => $emit('set', id, value)"
          @toggle="$emit('toggle', control.id, $event)"
          @fire="$emit('fire', control.id)"
          @note-create="
            (note) =>
              $emit(
                'noteCreate',
                control.id,
                control.kind === 'piano_roll' ? control.pattern : 0,
                note,
              )
          "
          @note-update="
            (note) =>
              $emit(
                'noteUpdate',
                control.id,
                control.kind === 'piano_roll' ? control.pattern : 0,
                note,
              )
          "
          @note-delete="
            (id) =>
              $emit(
                'noteDelete',
                control.id,
                control.kind === 'piano_roll' ? control.pattern : 0,
                id,
              )
          "
          @pattern="
            (pattern, boundary) =>
              $emit('pattern', control.id, pattern, boundary)
          "
          @adsr="
            (parameter, value) => $emit('adsr', control.id, parameter, value)
          "
        />
      </article>
    </div>
  </section>
</template>
