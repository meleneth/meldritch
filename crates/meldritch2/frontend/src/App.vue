<script setup lang="ts">
import { computed, ref, onMounted, watch } from "vue";
import { usePerformanceStore } from "./stores/performance";
import PerformancePlane from "./components/PerformancePlane.vue";
import type {
  AdsrParameter,
  NoteEvent,
  PatternControl,
} from "./types/protocol";
const store = usePerformanceStore(),
  credential = ref(""),
  activePlaneId = ref("");
const activePlane = computed(() =>
  store.visiblePlanes.find((plane) => plane.id === activePlaneId.value),
);
watch(
  () => store.visiblePlanes.map((plane) => plane.id),
  (ids) => {
    if (!ids.includes(activePlaneId.value)) activePlaneId.value = ids[0] || "";
  },
  { immediate: true },
);
onMounted(() => store.connect());
const set = (plane: string, id: string, value: number) => (
  store.patchRange(plane, id, value),
  store.send({ type: "control.set", name: id, value }, id, value)
);
const setToggle = (plane: string, id: string, value: boolean) =>
  store.send({ type: "control.set", name: id, value }, id, value);
const toggle = (plane: string, id: string, step: number) =>
  store.send({ type: "step.toggle", plane, control: id, step });
const fire = (plane: string, id: string) =>
  store.send({ type: "trigger.fire", plane, control: id });
const createNote = (
  plane: string,
  control: string,
  pattern: number,
  note: Omit<NoteEvent, "id">,
) => store.send({ type: "note.create", plane, control, pattern, ...note });
const updateNote = (
  plane: string,
  control: string,
  pattern: number,
  note: NoteEvent,
) => store.send({ type: "note.update", plane, control, pattern, ...note });
const deleteNote = (
  plane: string,
  control: string,
  pattern: number,
  id: number,
) => store.send({ type: "note.delete", plane, control, pattern, id });
const selectPattern = (
  plane: string,
  control: string,
  pattern: number,
  boundary: PatternControl["boundary"],
) => store.send({ type: "pattern.select", plane, control, pattern, boundary });
const setAdsr = (
  plane: string,
  control: string,
  parameter: AdsrParameter["name"],
  value: number,
) => store.send({ type: "adsr.set", plane, control, parameter, value });
const selectPreset = (
  plane: string,
  control: string,
  preset: number | string,
) => store.send({ type: "preset.select", plane, control, preset });
const toggleDrum = (
  plane: string,
  control: string,
  row: string,
  pattern: number,
  step: number,
) =>
  store.send({ type: "drum.hit.toggle", plane, control, row, pattern, step });
const setDrumVelocity = (
  plane: string,
  control: string,
  row: string,
  pattern: number,
  step: number,
  velocity: number,
) =>
  store.send({
    type: "drum.hit.velocity",
    plane,
    control,
    row,
    pattern,
    step,
    velocity,
  });
const muteDrum = (
  plane: string,
  control: string,
  row: string,
  muted: boolean,
) => store.send({ type: "drum.mute", plane, control, row, muted });
</script>
<template>
  <header class="top">
    <div>
      <p>MELDRITCH / LIVE</p>
      <h1>{{ store.name }}</h1>
    </div>
    <div class="session">
      <span :class="store.status">{{ store.status }}</span
      ><span>{{ store.role || "guest" }} · r{{ store.revision }}</span
      ><span
        class="xrun-count"
        :class="{ alarm: (store.audio?.buffer_underruns || 0) > 0 }"
        :title="store.audio?.first_stream_error || 'No audio backend errors'"
        >XRUNS {{ store.audio?.buffer_underruns || 0 }}</span
      ><input
        v-model="credential"
        type="password"
        placeholder="performer credential"
      /><button @click="store.authenticate(credential)">Join</button>
    </div>
  </header>
  <nav v-if="store.visiblePlanes.length" class="instrument-tabs" role="tablist">
    <button
      v-for="plane in store.visiblePlanes"
      :id="`tab-${plane.id}`"
      :key="plane.id"
      type="button"
      role="tab"
      :aria-controls="`panel-${plane.id}`"
      :aria-selected="plane.id === activePlaneId"
      :class="{ active: plane.id === activePlaneId }"
      @click="activePlaneId = plane.id"
    >
      {{ plane.label }}
    </button>
  </nav>
  <main>
    <PerformancePlane
      v-if="activePlane"
      :id="`panel-${activePlane.id}`"
      :key="activePlane.id"
      :plane="activePlane"
      role="tabpanel"
      :aria-labelledby="`tab-${activePlane.id}`"
      @set="(id, value) => set(activePlaneId, id, value)"
      @set-toggle="(id, value) => setToggle(activePlaneId, id, value)"
      @toggle="(id, step) => toggle(activePlaneId, id, step)"
      @fire="(id) => fire(activePlaneId, id)"
      @note-create="
        (id, pattern, note) => createNote(activePlaneId, id, pattern, note)
      "
      @note-update="
        (id, pattern, note) => updateNote(activePlaneId, id, pattern, note)
      "
      @note-delete="
        (id, pattern, noteId) => deleteNote(activePlaneId, id, pattern, noteId)
      "
      @pattern="
        (id, pattern, boundary) =>
          selectPattern(activePlaneId, id, pattern, boundary)
      "
      @adsr="
        (id, parameter, value) => setAdsr(activePlaneId, id, parameter, value)
      "
      @preset="(id, preset) => selectPreset(activePlaneId, id, preset)"
      @drum-toggle="
        (id, row, pattern, step) =>
          toggleDrum(activePlaneId, id, row, pattern, step)
      "
      @drum-velocity="
        (id, row, pattern, step, velocity) =>
          setDrumVelocity(activePlaneId, id, row, pattern, step, velocity)
      "
      @drum-mute="(id, row, muted) => muteDrum(activePlaneId, id, row, muted)"
    />
    <p v-if="!store.visiblePlanes.length" class="empty">
      Waiting for an authorized control plane…
    </p>
  </main>
  <aside v-if="store.errors.length">{{ store.errors.at(-1) }}</aside>
</template>
