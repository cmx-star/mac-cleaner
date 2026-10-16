<template>
  <div v-if="loading" class="page center">
    <Spinner />
  </div>

  <div v-else class="page" style="padding-bottom: 90px">
    <div class="title-row">
      <div>
        <div class="page-title">{{ t("optimize.title") }}</div>
        <div class="page-desc" style="margin-top: 4px; color: var(--text-dim); font-size: 13px">
          {{ t("optimize.subtitle") }}
        </div>
      </div>
      <button
        class="icon-button"
        :disabled="runningAll || runningTaskId !== null"
        :title="t('common.refresh')"
        @click="loadTasks"
      >
        <RefreshIcon />
      </button>
    </div>

    <div v-if="lastBatchMessage" class="clean-result-banner" style="margin-top: 12px">
      <div style="display: flex; alignItems: center; gap: 10px">
        <span style="color: var(--green); font-size: 16px">✓</span>
        <span style="font-size: 13px; font-weight: 500">{{ lastBatchMessage }}</span>
      </div>
      <button
        class="text-button"
        style="font-size: 12px; color: var(--text-dim); cursor: pointer; background: none; border: none"
        @click="lastBatchMessage = null"
      >
        ✕
      </button>
    </div>

    <div style="display: flex; flex-direction: column; gap: 12px; margin-top: 18px">
      <div
        v-for="task in tasks"
        :key="task.id"
        :class="['optimize-card', selectedIds.has(task.id) ? 'checked' : '']"
        :style="{
          background: 'var(--bg-panel)',
          border: selectedIds.has(task.id)
            ? '1px solid rgba(91, 163, 245, 0.45)'
            : '1px solid var(--border)',
          borderRadius: 'var(--radius)',
          padding: '16px 20px',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: '16px',
          transition: 'all 0.15s ease',
        }"
      >
        <!-- 左侧勾选与图标 -->
        <div style="display: flex; align-items: center; gap: 16px; flex: 1; min-width: 0">
          <input
            type="checkbox"
            :checked="selectedIds.has(task.id)"
            :disabled="runningAll || runningTaskId !== null"
            style="width: 17px; height: 17px; accent-color: var(--accent); cursor: pointer"
            @change="toggleSelect(task.id)"
          />

          <div
            :style="{
              width: '42px',
              height: '42px',
              borderRadius: '10px',
              background: selectedIds.has(task.id)
                ? 'rgba(91, 163, 245, 0.15)'
                : 'rgba(255, 255, 255, 0.05)',
              color: selectedIds.has(task.id) ? 'var(--accent)' : 'var(--text-dim)',
              display: 'grid',
              placeItems: 'center',
              flexShrink: 0,
            }"
          >
            <!-- 任务图标 -->
            <svg
              v-if="task.icon_name === 'globe'"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <circle cx="12" cy="12" r="10" />
              <line x1="2" y1="12" x2="22" y2="12" />
              <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
            </svg>
            <svg
              v-else-if="task.icon_name === 'trash'"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <polyline points="3 6 5 6 21 6" />
              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
            </svg>
            <svg
              v-else-if="task.icon_name === 'eye'"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
            <svg
              v-else-if="task.icon_name === 'grid'"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <rect x="3" y="3" width="7" height="7" />
              <rect x="14" y="3" width="7" height="7" />
              <rect x="14" y="14" width="7" height="7" />
              <rect x="3" y="14" width="7" height="7" />
            </svg>
            <svg
              v-else-if="task.icon_name === 'clipboard'"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
              <rect x="8" y="2" width="8" height="4" rx="1" ry="1" />
            </svg>
            <svg
              v-else-if="task.icon_name === 'type'"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <polyline points="4 7 4 4 20 4 20 7" />
              <line x1="9" y1="20" x2="15" y2="20" />
              <line x1="12" y1="4" x2="12" y2="20" />
            </svg>
            <span v-else>⚡</span>
          </div>

          <div style="flex: 1; min-width: 0">
            <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 4px">
              <span style="font-size: 14px; font-weight: 600; color: var(--text)">
                {{ t(task.name_key as MessageKey) }}
              </span>
              <span
                v-if="task.recommended"
                style="
                  font-size: 10px;
                  padding: 1px 6px;
                  border-radius: 4px;
                  background: rgba(76, 195, 138, 0.15);
                  color: var(--green);
                  font-weight: 500;
                "
              >
                {{ t("optimize.recommended") }}
              </span>
            </div>
            <div style="font-size: 12px; color: var(--text-dim); line-height: 1.45">
              {{ t(task.desc_key as MessageKey) }}
            </div>

            <div
              v-if="results[task.id]"
              :style="{
                marginTop: '6px',
                fontSize: '12px',
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
                color: results[task.id].success ? 'var(--green)' : 'var(--red)',
              }"
            >
              <span>{{ results[task.id].success ? "✓" : "✕" }}</span>
              <span>{{ results[task.id].message }}</span>
            </div>
          </div>
        </div>

        <!-- 右侧单个操作按钮 -->
        <div>
          <button
            class="secondary-button"
            :disabled="runningAll || runningTaskId !== null"
            style="
              padding: 6px 14px;
              border-radius: 8px;
              font-size: 12px;
              font-weight: 500;
              cursor: pointer;
              display: flex;
              align-items: center;
              gap: 6px;
              background: rgba(255, 255, 255, 0.06);
              border: 1px solid var(--border);
              color: var(--text);
            "
            @click="runSingle(task.id)"
          >
            <InlineSpinner v-if="runningTaskId === task.id || (runningAll && selectedIds.has(task.id))" />
            {{
              runningTaskId === task.id || (runningAll && selectedIds.has(task.id))
                ? t("optimize.running")
                : t("optimize.runSingle")
            }}
          </button>
        </div>
      </div>
    </div>

    <!-- 底部悬浮控制栏 -->
    <div class="clean-sticky-footer">
      <div class="clean-footer-left">
        <span>
          {{ t("clean.selectedCount", { count: selectedIds.size }) }}
        </span>
        <span>·</span>
        <button
          type="button"
          :class="['clean-action-link', tasks.length > 0 && selectedIds.size === tasks.length ? 'active' : '']"
          :disabled="runningAll || runningTaskId !== null"
          @click="selectAll"
        >
          {{ t("optimize.all") }}
        </button>
        <span>·</span>
        <button
          type="button"
          :class="['clean-action-link', selectedIds.size === 0 ? 'active' : '']"
          :disabled="runningAll || runningTaskId !== null"
          @click="selectNone"
        >
          {{ t("optimize.none") }}
        </button>
        <span>·</span>
        <button
          type="button"
          :class="[
            'clean-action-link',
            tasks.length > 0 &&
            tasks.filter((t) => t.recommended).every((t) => selectedIds.has(t.id)) &&
            selectedIds.size === tasks.filter((t) => t.recommended).length
              ? 'active'
              : '',
          ]"
          :disabled="runningAll || runningTaskId !== null"
          @click="selectRecommended"
        >
          {{ t("optimize.recommended") }}
        </button>
      </div>

      <div class="clean-footer-right">
        <button
          type="button"
          class="clean-submit-button"
          :disabled="selectedIds.size === 0 || runningAll || runningTaskId !== null"
          @click="runSelected"
        >
          <InlineSpinner v-if="runningAll" />
          <span v-else>⚡</span>
          <span>
            {{
              runningAll
                ? t("optimize.running")
                : t("optimize.runSelected", { count: selectedIds.size })
            }}
          </span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { listOptimizeTasks, runOptimizeTasks } from "../api";
import { useT, type MessageKey } from "../i18n";
import { Spinner, InlineSpinner } from "../components/Loading";
import { RefreshIcon } from "../components/Icons";
import type { OptimizeTaskDef, OptimizeTaskResult } from "../types";

const t = useT();
const tasks = ref<OptimizeTaskDef[]>([]);
const loading = ref(true);
const selectedIds = ref<Set<string>>(new Set());
const runningTaskId = ref<string | null>(null);
const runningAll = ref(false);
const results = ref<Record<string, OptimizeTaskResult>>({});
const lastBatchMessage = ref<string | null>(null);

async function loadTasks() {
  loading.value = true;
  try {
    const list = await listOptimizeTasks();
    tasks.value = list;
    const rec = new Set(list.filter((x) => x.recommended).map((x) => x.id));
    selectedIds.value = rec;
  } catch (e) {
    console.error("加载优化任务定义失败", e);
  } finally {
    loading.value = false;
  }
}

function toggleSelect(id: string) {
  const next = new Set(selectedIds.value);
  if (next.has(id)) {
    next.delete(id);
  } else {
    next.add(id);
  }
  selectedIds.value = next;
}

function selectAll() {
  selectedIds.value = new Set(tasks.value.map((t) => t.id));
}

function selectNone() {
  selectedIds.value = new Set();
}

function selectRecommended() {
  selectedIds.value = new Set(tasks.value.filter((t) => t.recommended).map((t) => t.id));
}

async function runSingle(id: string) {
  if (runningTaskId.value || runningAll.value) return;
  runningTaskId.value = id;
  lastBatchMessage.value = null;
  try {
    const res = await runOptimizeTasks([id]);
    if (res && res.length > 0) {
      results.value = { ...results.value, [id]: res[0] };
    }
  } catch (e) {
    results.value = {
      ...results.value,
      [id]: { id, success: false, message: String(e) },
    };
  } finally {
    runningTaskId.value = null;
  }
}

async function runSelected() {
  if (selectedIds.value.size === 0 || runningAll.value || runningTaskId.value) return;
  runningAll.value = true;
  lastBatchMessage.value = null;
  const targetIds = Array.from(selectedIds.value);
  try {
    const taskResults = await runOptimizeTasks(targetIds);
    const newMap: Record<string, OptimizeTaskResult> = {};
    let successCount = 0;
    for (const r of taskResults) {
      newMap[r.id] = r;
      if (r.success) successCount++;
    }
    results.value = { ...results.value, ...newMap };
    lastBatchMessage.value = t("optimize.successCount", { count: successCount });
  } catch (e) {
    lastBatchMessage.value = `执行失败：${String(e)}`;
  } finally {
    runningAll.value = false;
  }
}

onMounted(() => {
  loadTasks();
});
</script>
