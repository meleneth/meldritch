<script setup lang="ts">
import type {
  AdsrParameter,
  ControlDefinition,
  NoteEvent,
  PatternControl,
  PresetControl,
  DrumGridControl,
} from "../types/protocol";
import Adsr from "./controls/AdsrControl.vue";
import Fader from "./controls/FaderControl.vue";
import Knob from "./controls/KnobControl.vue";
import Patterns from "./controls/PatternSelector.vue";
import PianoRoll from "./controls/PianoRoll.vue";
import Presets from "./controls/PresetSelector.vue";
import Steps from "./controls/StepSequencer.vue";
import Trigger from "./controls/TriggerControl.vue";
import DrumGrid from "./controls/DrumGrid.vue";
defineProps<{ control: ControlDefinition }>();
defineEmits<{
  set: [string, number];
  adsr: [AdsrParameter["name"], number];
  toggle: [number];
  fire: [];
  noteCreate: [Omit<NoteEvent, "id">];
  noteUpdate: [NoteEvent];
  noteDelete: [number];
  pattern: [number, PatternControl["boundary"]];
  preset: [number];
  drumToggle: [string, number];
  drumVelocity: [string, number, number];
  drumMute: [string, boolean];
}>();
</script>
<template>
  <Steps
    v-if="control.kind === 'step_sequencer'"
    :control="control"
    @toggle="$emit('toggle', $event)"
  /><Fader
    v-else-if="control.kind === 'fader'"
    :control="control"
    @change="$emit('set', control.id, $event)"
  /><Knob
    v-else-if="control.kind === 'knob'"
    :control="control"
    @change="$emit('set', control.id, $event)"
  /><Trigger
    v-else-if="control.kind === 'trigger'"
    :control="control"
    @fire="$emit('fire')"
  />
  <PianoRoll
    v-else-if="control.kind === 'piano_roll'"
    :control="control"
    @create="$emit('noteCreate', $event)"
    @update="$emit('noteUpdate', $event)"
    @delete="$emit('noteDelete', $event)"
  />
  <DrumGrid
    v-else-if="control.kind === 'drum_grid'"
    :control="control as DrumGridControl"
    @toggle="(row, step) => $emit('drumToggle', row, step)"
    @velocity="(row, step, velocity) => $emit('drumVelocity', row, step, velocity)"
    @mute="(row, muted) => $emit('drumMute', row, muted)"
  />
  <Adsr
    v-else-if="control.kind === 'adsr'"
    :control="control"
    @change="(name, value) => $emit('adsr', name, value)"
  />
  <Patterns
    v-else-if="control.kind === 'pattern_selector'"
    :control="control"
    @select="(pattern, boundary) => $emit('pattern', pattern, boundary)"
  />
  <Presets
    v-else-if="control.kind === 'preset_selector'"
    :control="control as PresetControl"
    @select="$emit('preset', $event)"
  />
</template>
