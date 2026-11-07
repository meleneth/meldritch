<script setup lang="ts">
import { ref, onMounted } from "vue";
import { usePerformanceStore } from "./stores/performance";
import PerformancePlane from "./components/PerformancePlane.vue";
import type {
  AdsrParameter,
  NoteEvent,
  PatternControl,
} from "./types/protocol";
const store = usePerformanceStore(),
  credential = ref("");
onMounted(() => store.connect());
const set = (plane: string, id: string, value: number) =>
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
const selectPreset = (plane: string, control: string, preset: number) =>
  store.send({ type: "preset.select", plane, control, preset });
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
      ><input
        v-model="credential"
        type="password"
        placeholder="performer credential"
      /><button @click="store.authenticate(credential)">Join</button>
    </div>
  </header>
  <main>
    <PerformancePlane
      v-for="plane in store.visiblePlanes"
      :key="plane.id"
      :plane="plane"
      @set="(id, value) => set(plane.id, id, value)"
      @toggle="(id, step) => toggle(plane.id, id, step)"
      @fire="(id) => fire(plane.id, id)"
      @note-create="
        (id, pattern, note) => createNote(plane.id, id, pattern, note)
      "
      @note-update="
        (id, pattern, note) => updateNote(plane.id, id, pattern, note)
      "
      @note-delete="
        (id, pattern, noteId) => deleteNote(plane.id, id, pattern, noteId)
      "
      @pattern="
        (id, pattern, boundary) =>
          selectPattern(plane.id, id, pattern, boundary)
      "
      @adsr="(id, parameter, value) => setAdsr(plane.id, id, parameter, value)"
      @preset="(id, preset) => selectPreset(plane.id, id, preset)"
    />
    <p v-if="!store.visiblePlanes.length" class="empty">
      Waiting for an authorized control plane…
    </p>
  </main>
  <aside v-if="store.errors.length">{{ store.errors.at(-1) }}</aside>
</template>
