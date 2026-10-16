<template>
  <div v-if="cores.length > 0" class="core-bars" :title="cores.map((c) => `${c.toFixed(0)}%`).join('  ')">
    <div
      v-for="(c, i) in cores"
      :key="i"
      :class="['core-bar', c > 90 ? 'danger' : c > 75 ? 'warn' : '']"
      :style="{ height: `${Math.max(c, 3)}%` }"
    />
  </div>
</template>

<script setup lang="ts">
defineProps<{
  cores: number[];
}>();
</script>
