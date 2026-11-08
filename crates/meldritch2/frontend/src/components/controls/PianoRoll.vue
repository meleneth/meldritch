<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import PhaserControlSurface from "../PhaserControlSurface.vue";
import { createSurfaceBinding } from "../../control-surfaces/binding";
import { createPianoSurface } from "../../control-surfaces/piano";
import {
  deleteNotes,
  invertPitch,
  quantizePattern,
  rotatePattern,
  scaleTime,
  shiftTime,
  transpose,
  type PatternModel,
  type PatternNote,
  type ScaleMode,
} from "../../control-surfaces/pattern";
import { usePerformanceStore } from "../../stores/performance";
import type { NoteEvent, PianoRollControl } from "../../types/protocol";

const props = defineProps<{ control: PianoRollControl; planeId: string }>();
const emit = defineEmits<{
  create: [Omit<NoteEvent, "id">];
  update: [NoteEvent];
  delete: [number];
}>();
const store = usePerformanceStore();
const tonic = ref(0),
  mode = ref<ScaleMode>("chromatic"),
  chordRoot = ref(0),
  selected = ref(new Set<number>());
const toModel = (): PatternModel => ({
  length: props.control.step_count,
  resolution: 1,
  notes: props.control.notes.map((n) => ({
    id: n.id,
    start: n.step,
    duration: n.duration,
    pitch: n.pitch,
    velocity: n.velocity,
    legato: n.legato,
  })),
});
const toEvent = (n: PatternNote): NoteEvent => ({
  id: n.id,
  step: Math.round(n.start),
  duration: Math.max(1, Math.round(n.duration)),
  pitch: n.pitch,
  velocity: n.velocity,
  legato: n.legato ?? false,
});
const binding = createSurfaceBinding<PatternModel>({
  read: toModel,
  readOnly: !!props.control.read_only,
  equals: (a, b) => JSON.stringify(a) === JSON.stringify(b),
  subscribe: (listener) =>
    watch(
      () => props.control.notes,
      () => listener(toModel()),
    ),
  optimistic: (model) =>
    store.patchNotes(props.planeId, props.control.id, model.notes.map(toEvent)),
  write(model) {
    const before = new Map(props.control.notes.map((n) => [n.id, n]));
    const after = new Map(model.notes.map((n) => [n.id, toEvent(n)]));
    for (const [id] of before) if (!after.has(id)) emit("delete", id);
    for (const [id, note] of after) {
      const old = before.get(id);
      if (!old) {
        const { id: _id, ...created } = note;
        emit("create", created);
      } else if (JSON.stringify(old) !== JSON.stringify(note))
        emit("update", note);
    }
  },
});
const surface = computed(() =>
  createPianoSurface({
    id: `${props.planeId}-${props.control.id}`,
    label: props.control.label,
    pitchMin: props.control.pitch_min,
    pitchMax: props.control.pitch_max,
    scale: () => ({
      tonic: tonic.value,
      mode: mode.value,
      chordRoot: chordRoot.value,
      pitchMin: props.control.pitch_min,
      pitchMax: props.control.pitch_max,
    }),
    nextId: () => Math.max(0, ...props.control.notes.map((n) => n.id)) + 1,
    onSelection: (ids) => (selected.value = new Set(ids)),
  }),
);
const apply = (transform: (pattern: PatternModel) => PatternModel) =>
  binding.write(transform(binding.read()));
const removeSelected = () => {
  if (selected.value.size) apply((p) => deleteNotes(p, selected.value));
  selected.value = new Set();
};
onBeforeUnmount(() => binding.dispose());
</script>

<template>
  <div class="piano-roll phaser-editor">
    <header>
      <strong>{{ control.label }}</strong
      ><small
        >{{ control.voices }} voices · pattern {{ control.pattern + 1 }}</small
      >
    </header>
    <div class="surface-toolbar">
      <label
        >Tonic<select v-model.number="tonic">
          <option
            v-for="(name, index) in [
              'C',
              'C♯',
              'D',
              'E♭',
              'E',
              'F',
              'F♯',
              'G',
              'A♭',
              'A',
              'B♭',
              'B',
            ]"
            :key="name"
            :value="index"
          >
            {{ name }}
          </option>
        </select></label
      >
      <label
        >Mode<select v-model="mode">
          <option
            v-for="name in [
              'chromatic',
              'major',
              'minor',
              'dorian',
              'mixolydian',
              'pentatonic',
            ]"
            :key="name"
          >
            {{ name }}
          </option>
        </select></label
      >
      <label
        >Chord degree<select v-model.number="chordRoot">
          <option v-for="degree in 7" :key="degree" :value="degree - 1">
            {{ degree }}
          </option>
        </select></label
      >
      <button
        type="button"
        :disabled="control.read_only"
        @click="apply((p) => transpose(p, 1))"
      >
        + semitone
      </button>
      <button
        type="button"
        :disabled="control.read_only"
        @click="apply((p) => shiftTime(p, 1))"
      >
        Shift
      </button>
      <button
        type="button"
        :disabled="control.read_only"
        @click="apply((p) => rotatePattern(p, 1))"
      >
        Rotate
      </button>
      <button
        type="button"
        :disabled="control.read_only"
        @click="apply((p) => scaleTime(p, 0.5))"
      >
        Half time
      </button>
      <button
        type="button"
        :disabled="control.read_only"
        @click="apply((p) => invertPitch(p))"
      >
        Invert
      </button>
      <button
        type="button"
        :disabled="control.read_only"
        @click="apply((p) => quantizePattern(p))"
      >
        Quantize
      </button>
      <button
        type="button"
        :disabled="control.read_only || !selected.size"
        @click="removeSelected"
      >
        Delete selected
      </button>
    </div>
    <PhaserControlSurface
      :surface="surface"
      :binding="binding"
      :viewport="{ minZoomX: 8, maxZoomX: 160, minZoomY: 12, maxZoomY: 64 }"
      :height="360"
    />
    <small
      >Tap to add/select. Drag to move; drag the right edge to resize. Two
      fingers pan and pinch.</small
    >
  </div>
</template>
