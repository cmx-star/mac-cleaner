<template>
  <div class="gauge">
    <div
      :class="['gauge-fill', percent > 85 ? 'danger' : percent > 65 ? 'warn' : '']"
      :style="{ width: `${Math.min(Math.max(percent, 0), 100)}%` }"
    />
  </div>
</template>

<script setup lang="ts">
defineProps<{
  percent: number;
}>();
</script>
