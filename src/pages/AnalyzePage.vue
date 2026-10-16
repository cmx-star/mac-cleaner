<template>
  <div class="page">
    <div class="page-title">{{ t("analyze.title") }}</div>
    <div class="page-subtitle">{{ t("analyze.subtitle") }}</div>

    <form class="toolbar" @submit.prevent="handleSubmit">
      <input
        v-model="path"
        class="input"
        :placeholder="t('analyze.placeholder')"
        :disabled="scanning"
      />
      <button class="button" type="submit" :disabled="scanning">
        <InlineSpinner v-if="scanning" />
        {{ scanning ? t("analyze.scanningShort") : t("analyze.go") }}
      </button>
    </form>

    <div v-if="history.length > 0" style="display: flex; gap: 6px; flex-wrap: wrap; margin-bottom: 16px">
      <button
        v-for="h in history"
        :key="h"
        class="button secondary"
        style="padding: 4px 10px; font-size: 11px"
        @click="analyze(h, false)"
      >
        {{ h.replace(/^\/Users\/[^/]+/, "~") }}
      </button>
    </div>

    <div v-if="error" class="error-box">{{ error }}</div>

    <ScanProgress
      v-if="scanning"
      :phases="[
        {
          step: t('analyze.scanning', { path: scanningPath }),
          detail: rotating ?? undefined,
        },
      ]"
      :active="0"
    />

    <template v-if="result && !scanning">
      <div class="panel">
        <div class="panel-title" style="display: flex; justify-content: space-between">
          <span>{{ result.path }}</span>
          <span>
            {{ humanBytes(result.total_size) }} · {{ result.total_files }} 项
            <template v-if="result.scan_status === 'partial'"> · {{ t("analyze.partial") }}</template>
          </span>
        </div>
        <div
          v-for="e in result.entries"
          :key="e.path"
          class="row"
          :style="{ cursor: e.is_dir ? 'pointer' : 'default' }"
          @click="navigate(e.path, e.is_dir)"
        >
          <div class="row-main">
            <div class="row-name">
              {{ e.is_dir ? "▸ " : "  " }}
              {{ e.name }}
              <span v-if="e.is_build_artifact" class="tag green">{{ t("analyze.buildArtifact") }}</span>
              <span v-if="e.protection_reason" class="tag red" :title="e.path">
                {{ e.protection_reason }}
              </span>
              <span v-if="e.age_days !== null && e.age_days >= 90" class="tag yellow">
                {{ t("analyze.stale", { days: e.age_days }) }}
              </span>
            </div>
          </div>
          <div class="row-size">{{ humanBytes(e.size) }}</div>
        </div>
      </div>

      <div style="display: flex; gap: 8px">
        <button class="button secondary" @click="goUp">
          {{ t("analyze.up") }}
        </button>
      </div>

      <div v-if="result.large_files.length > 0" class="panel" style="margin-top: 16px">
        <div class="panel-title">{{ t("analyze.largeFiles") }}</div>
        <div v-for="f in result.large_files" :key="f.path" class="row">
          <div class="row-main">
            <div class="row-name">{{ f.name }}</div>
            <div class="row-path">{{ f.path }}</div>
          </div>
          <div class="row-size">{{ humanBytes(f.size) }}</div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { homeDir, scanDirectory, humanBytes } from "../api";
import { useT } from "../i18n";
import { ScanProgress, InlineSpinner, useRotatingDetail } from "../components/Loading";
import type { ScanResult } from "../types";

const t = useT();
const path = ref("");
const result = ref<ScanResult | null>(null);
const history = ref<string[]>([]);
const error = ref<string | null>(null);
const scanning = ref(false);
const scanningPath = ref("");

const rotating = useRotatingDetail(
  () => (scanning.value ? ["扫描目录", "统计体积", "识别构建产物", "检查大文件"] : []),
  900
);

onMounted(() => {
  homeDir()
    .then((dir) => {
      if (dir && path.value === "") {
        path.value = dir;
      }
    })
    .catch(() => {});
});

async function analyze(target: string, pushHistory = true) {
  scanning.value = true;
  scanningPath.value = target;
  error.value = null;
  try {
    const r = await scanDirectory(target);
    result.value = r;
    if (pushHistory) {
      history.value = [...history.value.filter((p) => p !== target), target].slice(-8);
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    scanning.value = false;
  }
}

function handleSubmit() {
  const target = path.value.trim();
  if (!target) {
    error.value = t("analyze.emptyPath");
    return;
  }
  analyze(target);
}

function goUp() {
  if (!result.value) return;
  const parent = result.value.path.replace(/\/[^/]+$/, "") || "/";
  analyze(parent);
}

function navigate(target: string, isDir: boolean) {
  if (isDir) {
    analyze(target);
  }
}
</script>
