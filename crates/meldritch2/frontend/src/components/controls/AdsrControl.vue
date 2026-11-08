<script setup lang="ts">
import { computed, onBeforeUnmount, watch } from "vue";
import PhaserControlSurface from "../PhaserControlSurface.vue";
import { createSurfaceBinding } from "../../control-surfaces/binding";
import { createAdsrSurface } from "../../control-surfaces/adsr";
import type {
  EnvelopeRanges,
  EnvelopeValues,
} from "../../control-surfaces/envelope";
import { usePerformanceStore } from "../../stores/performance";
import type { AdsrControl } from "../../types/protocol";

const props = defineProps<{ control: AdsrControl; planeId: string }>();
const emit = defineEmits<{
  change: [AdsrControl["parameters"][number]["name"], number];
}>();
const store = usePerformanceStore();
const parameter = (name: keyof EnvelopeValues) =>
  props.control.parameters.find((item) => item.name === name)!;
const ranges = computed(
  () =>
    Object.fromEntries(
      (["attack", "decay", "sustain", "release"] as const).map((name) => {
        const item = parameter(name);
        return [name, { min: item.min, max: item.max, scale: item.scale }];
      }),
    ) as EnvelopeRanges,
);
const values = (): EnvelopeValues => ({
  attack: props.control.values.attack ?? parameter("attack").min,
  decay: props.control.values.decay ?? parameter("decay").min,
  sustain: props.control.values.sustain ?? parameter("sustain").min,
  release: props.control.values.release ?? parameter("release").min,
});
const binding = createSurfaceBinding<EnvelopeValues>({
  read: values,
  readOnly: !!props.control.read_only,
  equals: (a, b) => JSON.stringify(a) === JSON.stringify(b),
  subscribe: (listener) =>
    watch(
      () => props.control.values,
      () => listener(values()),
    ),
  optimistic: (value) =>
    store.patchAdsr(props.planeId, props.control.id, { ...value }),
  write(value) {
    const current = values();
    for (const name of ["attack", "decay", "sustain", "release"] as const)
      if (value[name] !== current[name]) emit("change", name, value[name]);
  },
});
const surface = computed(() =>
  createAdsrSurface({
    id: `${props.planeId}-${props.control.id}`,
    label: props.control.label,
    ranges: ranges.value,
  }),
);
function numeric(name: keyof EnvelopeValues, event: Event) {
  binding.write({
    ...binding.read(),
    [name]: Number((event.target as HTMLInputElement).value),
  });
}
onBeforeUnmount(() => binding.dispose());
</script>

<template>
  <div class="adsr phaser-editor">
    <strong>{{ control.label }}</strong>
    <PhaserControlSurface
      :surface="surface"
      :binding="binding"
      :viewport="{ minZoomX: 0.5, maxZoomX: 8 }"
      :height="250"
    />
    <div class="envelope-numbers">
      <label v-for="item in control.parameters" :key="item.name"
        ><span>{{ item.name }}</span
        ><input
          type="number"
          :min="item.min"
          :max="item.max"
          :step="item.scale === 'log' ? '0.001' : '0.01'"
          :value="control.values[item.name] ?? item.min"
          :disabled="control.read_only"
          @input="numeric(item.name, $event)"
        /><small>{{ item.unit }}</small></label
      >
    </div>
    <small>Gate on: green attack/decay/sustain · gate off: red release</small>
  </div>
</template>
