<script setup lang="ts">
import { computed, ref } from "vue";
import type {
  AdsrParameter,
  NoteEvent,
  PatternControl,
  Plane,
} from "../types/protocol";
import ControlRenderer from "./ControlRenderer.vue";
import XyPad from "./controls/XyPad.vue";
import type { RangeControl } from "../types/protocol";
const props = defineProps<{ plane: Plane }>();
const ranges = computed(() =>
  props.plane.controls.filter(
    (control): control is RangeControl =>
      control.kind === "fader" || control.kind === "knob",
  ),
);
const collapsed = ref(false);
defineEmits<{
  set: [string, number];
  setToggle: [string, boolean];
  toggle: [string, number];
  fire: [string];
  noteCreate: [string, number, Omit<NoteEvent, "id">];
  noteUpdate: [string, number, NoteEvent];
  noteDelete: [string, number, number];
  pattern: [string, number, PatternControl["boundary"]];
  adsr: [string, AdsrParameter["name"], number];
  preset: [string, number];
  drumToggle: [string, string, number, number];
  drumVelocity: [string, string, number, number, number];
  drumMute: [string, string, boolean];
}>();
</script>
<template>
  <section class="plane" :data-station="plane.id">
    <header>
      <div>
        <h2>{{ plane.label }}</h2>
        <small>{{ plane.id }}</small>
      </div>
      <button
        class="plane-collapse"
        type="button"
        :aria-expanded="!collapsed"
        :aria-label="`${collapsed ? 'Open' : 'Close'} ${plane.label}`"
        @click="collapsed = !collapsed"
      >
        {{ collapsed ? "+" : "−" }}
      </button>
    </header>
    <div v-show="!collapsed" class="plane-grid">
      <article v-for="control in plane.controls" :key="control.id">
        <ControlRenderer
          :key="`${control.id}-${!!control.read_only}`"
          :control="control"
          :plane-id="plane.id"
          @set="(id, value) => $emit('set', id, value)"
          @set-toggle="(id, value) => $emit('setToggle', id, value)"
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
          @preset="$emit('preset', control.id, $event)"
          @drum-toggle="
            (row, step) =>
              $emit(
                'drumToggle',
                control.id,
                row,
                control.kind === 'drum_grid' ? control.pattern : 0,
                step,
              )
          "
          @drum-velocity="
            (row, step, velocity) =>
              $emit(
                'drumVelocity',
                control.id,
                row,
                control.kind === 'drum_grid' ? control.pattern : 0,
                step,
                velocity,
              )
          "
          @drum-mute="(row, muted) => $emit('drumMute', control.id, row, muted)"
        />
      </article>
      <article v-if="ranges.length >= 2" class="control-surface-playground">
        <header>
          <div>
            <strong>Linked parameter surfaces</strong
            ><small>Two Phaser instances · one Pinia state</small>
          </div>
          <button
            type="button"
            :disabled="plane.read_only"
            @click="
              $emit(
                'set',
                ranges[0].id,
                ranges[0].value === ranges[0].min
                  ? ranges[0].max
                  : ranges[0].min,
              )
            "
          >
            External mutation
          </button>
        </header>
        <div class="linked-surfaces">
          <XyPad
            :plane-id="plane.id"
            :x="ranges[0]"
            :y="ranges[1]"
            instance="A"
            @set="(id, value) => $emit('set', id, value)"
          />
          <XyPad
            :plane-id="plane.id"
            :x="ranges[0]"
            :y="ranges[1]"
            instance="B"
            @set="(id, value) => $emit('set', id, value)"
          />
        </div>
      </article>
    </div>
  </section>
</template>
