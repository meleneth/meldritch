<script setup lang="ts">
import type { ControlDefinition } from "../types/protocol";
import Fader from "./controls/FaderControl.vue";
import Knob from "./controls/KnobControl.vue";
import Steps from "./controls/StepSequencer.vue";
import Trigger from "./controls/TriggerControl.vue";
defineProps<{ control: ControlDefinition }>();
defineEmits<{ set: [number]; toggle: [number]; fire: [] }>();
</script>
<template>
  <Steps
    v-if="control.kind === 'step_sequencer'"
    :control="control"
    @toggle="$emit('toggle', $event)"
  /><Fader
    v-else-if="control.kind === 'fader'"
    :control="control"
    @change="$emit('set', $event)"
  /><Knob
    v-else-if="control.kind === 'knob'"
    :control="control"
    @change="$emit('set', $event)"
  /><Trigger
    v-else-if="control.kind === 'trigger'"
    :control="control"
    @fire="$emit('fire')"
  />
</template>
