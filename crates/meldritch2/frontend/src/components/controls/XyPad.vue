<script setup lang="ts">
import { computed, onBeforeUnmount, watch } from "vue";
import PhaserControlSurface from "../PhaserControlSurface.vue";
import {
  composeBindings,
  createSurfaceBinding,
} from "../../control-surfaces/binding";
import { createXySurface } from "../../control-surfaces/xy";
import { usePerformanceStore } from "../../stores/performance";
import type { RangeControl } from "../../types/protocol";

const props = defineProps<{
  planeId: string;
  x: RangeControl;
  y: RangeControl;
  instance?: string;
}>();
const store = usePerformanceStore();
const emit = defineEmits<{ set: [string, number] }>();
const current = (id: string) => {
  const control = store.planes
    .find((plane) => plane.id === props.planeId)
    ?.controls.find((item) => item.id === id);
  return control?.kind === "fader" || control?.kind === "knob"
    ? control.value
    : id === props.x.id
      ? props.x.value
      : props.y.value;
};
function axis(control: RangeControl) {
  return createSurfaceBinding<number>({
    read: () => current(control.id),
    readOnly: !!control.read_only,
    subscribe: (listener) => watch(() => current(control.id), listener),
    optimistic: (value) => store.patchRange(props.planeId, control.id, value),
    write: (value) => emit("set", control.id, value),
  });
}
const binding = composeBindings({
  [props.x.id]: axis(props.x),
  [props.y.id]: axis(props.y),
});
const surface = computed(() =>
  createXySurface({
    id: `${props.planeId}-${props.instance ?? "primary"}`,
    label: `${props.x.label} by ${props.y.label}`,
    x: [
      {
        target: props.x.id,
        min: props.x.min,
        max: props.x.max,
        scale: props.x.scale,
      },
    ],
    y: [
      {
        target: props.y.id,
        min: props.y.min,
        max: props.y.max,
        scale: props.y.scale,
      },
    ],
  }),
);
onBeforeUnmount(() => binding.dispose());
</script>

<template>
  <section class="xy-pad">
    <header>
      <strong>{{ x.label }} × {{ y.label }}</strong
      ><small>{{ instance }}</small>
    </header>
    <PhaserControlSurface :surface="surface" :binding="binding" :height="260" />
    <footer>
      <output>X {{ current(x.id).toFixed(2) }}</output
      ><output>Y {{ current(y.id).toFixed(2) }}</output>
    </footer>
  </section>
</template>
