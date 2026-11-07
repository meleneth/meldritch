<script setup lang="ts">
import type { AdsrControl } from "../../types/protocol";
const props = defineProps<{ control: AdsrControl }>();
const emit = defineEmits<{
  change: [AdsrControl["parameters"][number]["name"], number];
}>();
const normalized = (name: string, min: number, max: number, scale: string) => {
  const value = props.control.values?.[name] ?? min;
  return scale === "log"
    ? (Math.log(value) - Math.log(min)) / (Math.log(max) - Math.log(min))
    : (value - min) / (max - min);
};
const mapped = (raw: number, min: number, max: number, scale: string) =>
  scale === "log"
    ? Math.exp(Math.log(min) + (Math.log(max) - Math.log(min)) * raw)
    : min + (max - min) * raw;
function change(
  name: AdsrControl["parameters"][number]["name"],
  min: number,
  max: number,
  scale: string,
  event: Event,
) {
  emit(
    "change",
    name,
    mapped(Number((event.target as HTMLInputElement).value), min, max, scale),
  );
}
</script>
<template>
  <div class="adsr">
    <strong>{{ control.label }}</strong>
    <svg viewBox="0 0 300 90" aria-hidden="true">
      <polyline points="5,85 55,8 115,55 220,55 295,85" />
    </svg>
    <label v-for="parameter in control.parameters" :key="parameter.name">
      <span>{{ parameter.name }}</span>
      <input
        type="range"
        min="0"
        max="1"
        step="0.001"
        :value="
          normalized(
            parameter.name,
            parameter.min,
            parameter.max,
            parameter.scale,
          )
        "
        :disabled="control.read_only"
        @input="
          change(
            parameter.name,
            parameter.min,
            parameter.max,
            parameter.scale,
            $event,
          )
        "
      />
      <output
        >{{
          (control.values?.[parameter.name] ?? parameter.min).toFixed(
            parameter.name === "sustain" ? 2 : 3,
          )
        }}
        {{ parameter.unit }}</output
      >
    </label>
  </div>
</template>
