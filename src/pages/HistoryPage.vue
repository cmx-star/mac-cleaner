<template>
  <div v-if="error" class="error-box">{{ error }}</div>
  <Spinner v-else-if="!history" :label="t('common.loading')" />
  <div v-else class="page">
    <div class="page-title">{{ t("history.title") }}</div>
    <div class="page-subtitle">{{ t("history.subtitle") }}</div>

    <div v-if="!hasContent" class="empty">{{ t("history.empty") }}</div>
    <template v-else>
      <div v-if="history.sessions.length > 0" class="panel">
        <div class="panel-title">{{ t("history.sessions") }}</div>
        <div v-for="(s, i) in history.sessions" :key="i" class="row">
          <div class="row-main">
            <div class="row-name">
              {{ s.command }}
              <span class="tag blue">{{ t("history.items", { count: s.items }) }}</span>
              <span v-if="s.failed > 0" class="tag red">{{ t("history.failed", { count: s.failed }) }}</span>
            </div>
            <div class="row-path">
              {{ s.started_at }}
              <template v-if="s.ended_at !== s.started_at"> → {{ s.ended_at }}</template>
            </div>
          </div>
          <div class="row-size">{{ s.size }}</div>
        </div>
      </div>

      <div v-if="history.deletions.length > 0" class="panel">
        <div class="panel-title">{{ t("history.deletions") }}</div>
        <div v-for="(d, i) in history.deletions" :key="i" class="row">
          <div class="row-main">
            <div class="row-path" style="font-size: 12px">{{ d.path || d.timestamp }}</div>
            <div class="row-path">
              {{ d.timestamp }} · {{ d.mode }}
              <template v-if="d.status !== 'ok'"> · {{ d.status }}</template>
            </div>
          </div>
          <div class="row-size">{{ d.size_display }}</div>
        </div>
      </div>
    </template>

    <div style="display: flex; justify-content: flex-end">
      <button class="button secondary" @click="load">
        {{ t("common.refresh") }}
      </button>
    </div>
    <div style="color: var(--text-dim); font-size: 11px; margin-top: 8px">
      {{ t("history.logAt", { path: history.operations_log }) }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { readHistory } from "../api";
import { useT } from "../i18n";
import { Spinner } from "../components/Loading";
import type { History } from "../types";

const t = useT();
const history = ref<History | null>(null);
const error = ref<string | null>(null);

const hasContent = computed(() => {
  return (history.value?.sessions.length ?? 0) > 0 || (history.value?.deletions.length ?? 0) > 0;
});

async function load() {
  try {
    history.value = await readHistory(50);
    error.value = null;
  } catch (e) {
    error.value = String(e);
  }
}

onMounted(() => {
  load();
});
</script>
