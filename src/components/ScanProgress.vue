<template>
  <div class="loading-block" role="status" aria-live="polite">
    <span class="spinner" aria-hidden="true" />
    <div class="loading-body">
      <div class="loading-label">{{ currentPhase?.step }}</div>
      <div v-if="currentPhase?.detail" class="loading-detail">{{ currentPhase.detail }}</div>
      <div v-if="currentPhase?.percent !== undefined" class="progress-track">
        <div class="progress-fill" :style="{ width: `${Math.min(currentPhase.percent, 100)}%` }" />
      </div>
      <div v-else class="progress-track indeterminate">
        <div class="progress-stripes" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

export interface ProgressPhase {
  /** 主步骤名，如「正在扫描可清理项」。 */
  step: string;
  /** 细节文案，如「正在检查 ~/Library/Caches」。 */
  detail?: string;
  /** 0-100。不传则显示流动条纹的「进行中」状态。 */
  percent?: number;
}

const props = defineProps<{
  phases: ProgressPhase[];
  active: number;
}>();

const currentPhase = computed(() => {
  return props.phases[Math.min(props.active, props.phases.length - 1)] ?? props.phases[0];
});
</script>
