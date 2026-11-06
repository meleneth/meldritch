<script setup lang="ts">
import { ref, onMounted } from "vue";
import { usePerformanceStore } from "./stores/performance";
import PerformancePlane from "./components/PerformancePlane.vue";
const store = usePerformanceStore(),
  credential = ref("");
onMounted(() => store.connect());
const set = (plane: string, id: string, value: number) =>
  store.send({ type: "control.set", name: id, value }, id, value);
const toggle = (plane: string, id: string, step: number) =>
  store.send({ type: "step.toggle", plane, control: id, step });
const fire = (plane: string, id: string) =>
  store.send({ type: "trigger.fire", plane, control: id });
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
    />
    <p v-if="!store.visiblePlanes.length" class="empty">
      Waiting for an authorized control plane…
    </p>
  </main>
  <aside v-if="store.errors.length">{{ store.errors.at(-1) }}</aside>
</template>
