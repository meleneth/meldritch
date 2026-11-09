<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { PresetControl } from "../../types/protocol";
const props = defineProps<{ control: PresetControl }>();
const emit = defineEmits<{ select: [string] }>();
const search = ref(""),
  bank = ref("All"),
  category = ref("All"),
  cursor = ref(0),
  start = ref(0);
const entries = computed(() =>
  props.control.entries?.length
    ? props.control.entries
    : props.control.presets.map((name) => ({
        id: name,
        name,
        bank: "Authored",
        category: "Uncategorized",
      })),
);
const banks = computed(() => [
  "All",
  ...new Set(entries.value.map((entry) => entry.bank)),
]);
const categories = computed(() => [
  "All",
  ...new Set(entries.value.map((entry) => entry.category)),
]);
const filtered = computed(() => {
  const needle = search.value.trim().toLocaleLowerCase();
  return entries.value.filter(
    (entry) =>
      (bank.value === "All" || entry.bank === bank.value) &&
      (category.value === "All" || entry.category === category.value) &&
      (!needle || entry.name.toLocaleLowerCase().includes(needle)),
  );
});
const visible = computed(() =>
  filtered.value.slice(start.value, start.value + 40),
);
const activeId = computed(() => entries.value[props.control.active]?.id);
watch([search, bank, category], () => {
  cursor.value = 0;
  start.value = 0;
});
function move(delta: number) {
  cursor.value = Math.max(
    0,
    Math.min(filtered.value.length - 1, cursor.value + delta),
  );
  if (cursor.value < start.value) start.value = cursor.value;
  if (cursor.value >= start.value + 40) start.value = cursor.value - 39;
}
function key(event: KeyboardEvent) {
  if (event.key === "ArrowDown") {
    event.preventDefault();
    move(1);
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    move(-1);
  } else if (event.key === "Enter" && filtered.value[cursor.value]) {
    event.preventDefault();
    emit("select", filtered.value[cursor.value].id);
  }
}
function scroll(event: Event) {
  const element = event.currentTarget as HTMLElement;
  const fraction =
    element.scrollTop /
    Math.max(1, element.scrollHeight - element.clientHeight);
  start.value = Math.round(fraction * Math.max(0, filtered.value.length - 40));
}
const previous = () =>
  emit(
    "select",
    entries.value[
      (props.control.active - 1 + entries.value.length) % entries.value.length
    ].id,
  );
const next = () =>
  emit(
    "select",
    entries.value[(props.control.active + 1) % entries.value.length].id,
  );
</script>

<template>
  <fieldset class="preset-browser" :disabled="control.read_only" @keydown="key">
    <legend>{{ control.label }}</legend>
    <div class="current">
      <button type="button" aria-label="Previous preset" @click="previous">
        ←</button
      ><strong>{{ control.active_name || "No preset loaded" }}</strong
      ><button type="button" aria-label="Next preset" @click="next">→</button
      ><span v-if="control.pending !== null" class="pending">loading…</span>
    </div>
    <div class="filters">
      <input
        v-model="search"
        type="search"
        placeholder="Search presets"
        aria-label="Search presets"
      /><select v-model="bank" aria-label="Preset bank">
        <option v-for="value in banks" :key="value">{{ value }}</option></select
      ><select v-model="category" aria-label="Preset category">
        <option v-for="value in categories" :key="value">{{ value }}</option>
      </select>
    </div>
    <div
      class="list"
      role="listbox"
      tabindex="0"
      :aria-activedescendant="filtered[cursor]?.id"
      @scroll="scroll"
    >
      <div :style="{ height: `${start * 42}px` }" />
      <button
        v-for="(preset, offset) in visible"
        :id="preset.id"
        :key="preset.id"
        type="button"
        role="option"
        :aria-selected="preset.id === activeId"
        :class="{
          active: preset.id === activeId,
          cursor: start + offset === cursor,
        }"
        @focus="cursor = start + offset"
        @click="$emit('select', preset.id)"
      >
        <span>{{ preset.name }}</span
        ><small>{{ preset.bank }} · {{ preset.category }}</small>
      </button>
      <div
        :style="{
          height: `${Math.max(0, filtered.length - start - visible.length) * 42}px`,
        }"
      />
    </div>
    <small>{{ filtered.length }} of {{ entries.length }} presets</small>
    <small v-if="control.failed" role="alert">{{ control.failed }}</small>
  </fieldset>
</template>

<style scoped>
.preset-browser {
  display: grid;
  gap: 0.55rem;
  min-width: 18rem;
  border: 0;
  padding: 0;
}
legend {
  font-weight: 700;
}
.current,
.filters {
  display: flex;
  gap: 0.45rem;
  align-items: center;
}
.current strong {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
input {
  min-width: 0;
  flex: 1;
}
.filters input,
.filters select,
.current button {
  min-height: 2.35rem;
  color: inherit;
  background: #17202e;
  border: 1px solid #687386;
  border-radius: 0.4rem;
}
.list {
  height: 13rem;
  overflow-y: auto;
  border: 1px solid #405068;
  border-radius: 0.45rem;
  background: #101722;
}
.list button {
  width: 100%;
  height: 42px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 0.5rem;
  padding: 0.35rem 0.6rem;
  border: 0;
  border-bottom: 1px solid #263348;
  background: transparent;
  color: inherit;
  text-align: left;
}
.list button.active {
  background: #16475b;
}
.list button.cursor {
  outline: 1px solid #66e3ff;
  outline-offset: -2px;
}
.list small {
  opacity: 0.68;
  white-space: nowrap;
}
.pending {
  color: #ffd166;
}
[role="alert"] {
  color: #ff7b7b;
}
</style>
