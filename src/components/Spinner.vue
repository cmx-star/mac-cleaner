<template>
  <div class="loading-block" role="status" aria-live="polite">
    <span class="spinner" aria-hidden="true" />
    <span v-if="label" class="loading-label">{{ label }}</span>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  label?: string;
}>();
</script>
