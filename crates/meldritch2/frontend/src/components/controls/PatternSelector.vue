<script setup lang="ts">
import { ref } from "vue";
import type { PatternControl } from "../../types/protocol";
const props = defineProps<{ control: PatternControl }>();
const emit = defineEmits<{ select: [number, PatternControl["boundary"]] }>();
const boundary = ref(props.control.default_boundary);
</script>
<template>
  <div class="patterns">
    <strong>{{ control.label }}</strong>
    <label
      >Switch at
      <select v-model="boundary" :disabled="control.read_only">
        <option v-for="item in control.boundaries" :key="item">
          {{ item }}
        </option>
      </select></label
    >
    <div>
      <button
        v-for="(pattern, index) in control.patterns"
        :key="pattern"
        :class="{
          active: index === control.active,
          pending: index === control.pending,
        }"
        :disabled="control.read_only"
        @click="emit('select', index, boundary)"
      >
        {{ pattern }}
      </button>
    </div>
    <small v-if="control.pending !== null"
      >{{ control.pending_name }} pending at {{ control.boundary }}</small
    >
    <small v-else>Playing {{ control.active_name }}</small>
  </div>
</template>
