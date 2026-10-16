<template>
  <div style="position: relative; width: 100%; flex: 1" :style="{ height: `${height}px`, minHeight: `${height}px` }">
    <svg
      class="sparkline"
      :viewBox="`0 0 ${w} ${height}`"
      preserveAspectRatio="none"
      style="width: 100%; height: 100%; display: block"
    >
      <defs>
        <linearGradient id="cpuTempAreaGrad" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" :stop-color="cpuColor" stop-opacity="0.28" />
          <stop offset="100%" :stop-color="cpuColor" stop-opacity="0.0" />
        </linearGradient>
      </defs>

      <!-- CPU 面积柔光 -->
      <polygon :points="areaPoints" fill="url(#cpuTempAreaGrad)" />

      <!-- GPU 走势曲线（次要，冷色调天蓝细虚线） -->
      <polyline
        v-if="hasGpu && gpuPoints"
        :points="gpuPoints"
        fill="none"
        stroke="#61afef"
        stroke-width="1.2"
        stroke-dasharray="3 2"
        opacity="0.85"
        vector-effect="non-scaling-stroke"
      />

      <!-- CPU 走势曲线（主曲线，鲜明实线） -->
      <polyline
        :points="cpuPoints"
        fill="none"
        :stroke="cpuColor"
        stroke-width="1.6"
        stroke-linecap="round"
        stroke-linejoin="round"
        vector-effect="non-scaling-stroke"
      />
    </svg>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    series: { cpu: number; gpu: number }[];
    currentCpu: number;
    currentGpu: number;
    height?: number;
  }>(),
  {
    height: 68,
  }
);

const w = 100;

const pointsData = computed(() => {
  return props.series.length >= 2
    ? props.series
    : [
        { cpu: props.currentCpu, gpu: props.currentGpu },
        { cpu: props.currentCpu, gpu: props.currentGpu },
      ];
});

const allTemps = computed(() => {
  const allCpu = pointsData.value.map((p) => p.cpu).filter((v) => v > 0);
  const allGpu = pointsData.value.map((p) => p.gpu).filter((v) => v > 0);
  return [...allCpu, ...allGpu];
});

const minVal = computed(() => (allTemps.value.length > 0 ? Math.min(...allTemps.value) : 40));
const maxVal = computed(() => (allTemps.value.length > 0 ? Math.max(...allTemps.value) : 80));

const yRangeData = computed(() => {
  const pad = Math.max((maxVal.value - minVal.value) * 0.2, 4);
  const yMin = Math.max(minVal.value - pad, 20);
  const yMax = maxVal.value + pad;
  const yRange = Math.max(yMax - yMin, 14);
  return { yMin, yRange };
});

function getY(v: number) {
  const { yMin, yRange } = yRangeData.value;
  const yMax = yMin + yRange;
  const clamped = Math.min(Math.max(v, yMin), yMax);
  return (props.height - ((clamped - yMin) / yRange) * (props.height - 10) - 5).toFixed(2);
}

const cpuColor = computed(() => {
  return props.currentCpu > 85 ? "var(--red)" : props.currentCpu > 70 ? "var(--yellow)" : "var(--green)";
});

const hasGpu = computed(() => props.currentGpu > 0);

const cpuPoints = computed(() => {
  const pts = pointsData.value;
  const step = w / (pts.length - 1);
  return pts.map((p, i) => `${(i * step).toFixed(2)},${getY(p.cpu || props.currentCpu)}`).join(" ");
});

const gpuPoints = computed(() => {
  if (!hasGpu.value) return null;
  const pts = pointsData.value;
  const step = w / (pts.length - 1);
  return pts.map((p, i) => `${(i * step).toFixed(2)},${getY(p.gpu || props.currentGpu)}`).join(" ");
});

const areaPoints = computed(() => {
  const pts = pointsData.value;
  const step = w / (pts.length - 1);
  return `0,${props.height} ${cpuPoints.value} ${(pts.length - 1) * step},${props.height}`;
});
</script>
