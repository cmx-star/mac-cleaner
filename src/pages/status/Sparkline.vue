<template>
  <div v-if="values.length < 2" class="sparkline" />
  <svg v-else class="sparkline" :viewBox="`0 0 ${w} ${h}`" preserveAspectRatio="none">
    <polyline
      :points="points"
      fill="none"
      stroke="var(--accent)"
      stroke-width="1.5"
      vector-effect="non-scaling-stroke"
    />
  </svg>
</template>

<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  values: number[];
}>();

const w = 100;
const h = 34;

const points = computed(() => {
  if (props.values.length < 2) return "";
  const max = Math.max(...props.values, 1e-6);
  const step = w / (props.values.length - 1);
  return props.values
    .map((v, i) => `${(i * step).toFixed(2)},${(h - (v / max) * h).toFixed(2)}`)
    .join(" ");
});
</script>
