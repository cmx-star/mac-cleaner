<template>
  <!-- 首次进入时的扫描态：标题下方显示进度，而不是空白页 -->
  <div v-if="!apps && !error" class="page">
    <div class="page-title">{{ t("uninstall.title") }}</div>
    <div class="page-subtitle">{{ t("uninstall.subtitleEntry") }}</div>
    <ScanProgress
      :phases="[
        {
          step: t('uninstall.scanning'),
          detail: rotatingApp ?? undefined,
        },
      ]"
      :active="0"
    />
  </div>

  <div v-else-if="error && !apps" class="error-box">{{ error }}</div>

  <div v-else :class="['page', selected ? 'has-action-bar' : '']">
    <div class="title-row">
      <div class="page-title">{{ t("uninstall.title") }}</div>
      <button
        class="icon-button"
        :disabled="scanning"
        :title="t('clean.rescan')"
        :aria-label="t('clean.rescan')"
        @click="load(true)"
      >
        <RefreshIcon :spinning="scanning" />
      </button>
    </div>
    <div class="page-subtitle">{{ t("uninstall.subtitle") }}</div>

    <div v-if="error" class="error-box">{{ error }}</div>

    <div class="toolbar">
      <div class="scope-tabs">
        <button
          :class="['scope-tab', scope === 'user' ? 'active' : '']"
          @click="
            scope = 'user';
            selected = null;
            confirming = false;
          "
        >
          {{ t("uninstall.tabUser") }}
          <span class="scope-count">{{ userCount }}</span>
        </button>
        <button
          :class="['scope-tab', scope === 'system' ? 'active' : '']"
          @click="
            scope = 'system';
            selected = null;
            confirming = false;
          "
        >
          {{ t("uninstall.tabSystem") }}
          <span class="scope-count">{{ systemCount }}</span>
        </button>
      </div>
      <input
        v-model="search"
        class="input"
        :placeholder="t('uninstall.searchPlaceholder')"
      />
    </div>

    <div class="panel">
      <div class="panel-title">{{ t("uninstall.installed", { count: filtered.length }) }}</div>
      <div v-if="filtered.length === 0" class="empty">
        {{ scope === "system" ? t("uninstall.emptySystem") : t("uninstall.emptyUser") }}
      </div>
      <div
        v-for="app in filtered"
        :key="app.path"
        class="row"
        :style="{
          cursor: 'pointer',
          background: selected?.path === app.path ? 'var(--bg-hover)' : undefined,
          borderRadius: '6px',
        }"
        @click="selectApp(app)"
      >
        <img
          v-if="app.icon_png_base64"
          class="app-icon"
          :src="`data:image/png;base64,${app.icon_png_base64}`"
          alt=""
          aria-hidden="true"
        />
        <span v-else class="app-icon app-icon-placeholder" aria-hidden="true">
          {{ app.name.slice(0, 1).toUpperCase() }}
        </span>

        <div class="row-main">
          <div class="row-name">
            {{ app.name }}
            <span v-if="app.version" class="row-path" style="display: inline; margin-left: 8px">
              {{ app.version }}
            </span>
            <span v-if="app.source === 'Homebrew'" class="tag blue">Homebrew</span>
            <span v-if="app.protected" class="tag red">{{ t("uninstall.protected") }}</span>
          </div>
          <div class="row-path">
            {{ app.bundle_id || app.path }}
            <span
              v-if="app.last_used_days_ago !== null"
              :style="{
                marginLeft: '8px',
                color: app.last_used_days_ago > 90 ? 'var(--yellow)' : 'var(--text-dim)',
              }"
            >
              {{
                app.last_used_days_ago === 0
                  ? t("uninstall.usedToday")
                  : app.last_used_days_ago < 30
                    ? t("uninstall.daysAgo", { days: app.last_used_days_ago })
                    : t("uninstall.monthsAgo", { months: Math.round(app.last_used_days_ago / 30) })
              }}
            </span>
          </div>
        </div>
        <div class="row-size">{{ app.size_display }}</div>
      </div>
    </div>

    <!-- 选中应用的数据目录明细 -->
    <div v-if="selected" class="panel">
      <div class="panel-title">
        {{ t("uninstall.appData", { name: selected.name }) }}
        <span v-if="selected.protected" class="tag red" style="margin-left: 8px">
          {{ selected.protection_reason }}
        </span>
      </div>

      <div v-if="loadingData" class="loading-block" style="padding: 20px 0">
        <Spinner :label="t('uninstall.selecting', { name: selected.name })" />
      </div>
      <div v-else-if="dataPaths.length === 0" class="empty">
        {{ t("uninstall.noData") }}
      </div>
      <template v-else>
        <div v-for="p in dataPaths" :key="p.path" class="row">
          <div class="row-main">
            <div class="row-path" style="font-size: 12px">{{ p.path }}</div>
            <div v-if="p.protection_reason" class="row-path" style="color: var(--yellow)">
              {{ p.protection_reason }}
            </div>
          </div>
          <div class="row-size">{{ p.size_display }}</div>
        </div>
      </template>

      <div style="display: flex; gap: 8px; justify-content: flex-end; margin-top: 12px">
        <span style="color: var(--text-dim); align-self: center; font-size: 11px">
          {{ selected.protected ? t("uninstall.protected") : t("uninstall.actionBarHint") }}
        </span>
      </div>

      <div v-if="running" class="loading-block" style="padding: 16px 0">
        <span class="spinner inline" aria-hidden="true" />
        <div class="loading-body">
          <span class="loading-label" style="font-size: 12px">
            {{ t("uninstall.running", { name: selected.name }) }}
          </span>
          <span class="loading-detail">{{ selected.path }}</span>
        </div>
      </div>
    </div>

    <!-- 命令行退出结果 -->
    <div v-if="result" class="panel">
      <div class="panel-title">
        {{ t("common.exitCode", { code: result.exit_code ?? "—" }) }}
      </div>
      <div class="output-box">{{ result.stdout || result.stderr || t("common.noOutput") }}</div>
    </div>

    <div v-if="apps" style="color: var(--text-dim); font-size: 11px; text-align: center; margin-top: 8px">
      {{
        t("common.appCount", {
          count: apps.length,
          size: humanBytes(apps.reduce((s, a) => s + a.size, 0)),
        })
      }}
      <template v-if="cacheAge !== null">
        · {{ t("uninstall.cachedAgo", { time: formatAge(cacheAge) }) }}
      </template>
    </div>

    <!-- 底部实体安全留白 -->
    <div :class="['bottom-spacer', selected ? 'with-action-bar' : '']" />

    <!-- 悬浮在窗口底部：选中应用后立刻可见 -->
    <div v-if="selected" class="action-bar">
      <div class="action-bar-inner">
        <img
          v-if="selected.icon_png_base64"
          class="app-icon action-bar-icon"
          :src="`data:image/png;base64,${selected.icon_png_base64}`"
          alt=""
          aria-hidden="true"
        />
        <span v-else class="app-icon app-icon-placeholder action-bar-icon" aria-hidden="true">
          {{ selected.name.slice(0, 1).toUpperCase() }}
        </span>

        <div class="action-bar-info">
          <span class="action-bar-name">{{ selected.name }}</span>
          <span class="action-bar-detail">
            {{
              running
                ? t("uninstall.running", { name: selected.name })
                : selected.protected
                  ? selected.protection_reason
                  : `${selected.size_display} · ${selected.path}`
            }}
          </span>
        </div>

        <button
          v-if="!confirming"
          class="button danger"
          :disabled="selected.protected || loadingData || running"
          @click="confirming = true"
        >
          {{ t("uninstall.remove", { name: selected.name }) }}
        </button>
        <template v-else>
          <button class="button secondary" :disabled="running" @click="confirming = false">
            {{ t("common.cancel") }}
          </button>
          <button class="button danger" :disabled="running" @click="uninstall">
            <InlineSpinner v-if="running" />
            {{ running ? t("uninstall.running", { name: selected.name }) : t("uninstall.confirmTip") }}
          </button>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { appData, runUninstall, humanBytes } from "../api";
import { useT } from "../i18n";
import { Spinner, InlineSpinner, ScanProgress, useRotatingDetail } from "../components/Loading";
import { RefreshIcon } from "../components/Icons";
import { appsCacheAge, invalidateAppsCache, loadApps, peekAppsCache } from "../appsCache";
import type { App, AppDataPath, CommandOutput } from "../types";

function formatAge(ms: number): string {
  const minutes = Math.floor(ms / 60000);
  if (minutes < 1) return "<1m";
  if (minutes < 60) return `${minutes}m`;
  return `${Math.floor(minutes / 60)}h`;
}

const t = useT();
const apps = ref<App[] | null>(peekAppsCache());
const error = ref<string | null>(null);
const search = ref("");
const selected = ref<App | null>(null);
const dataPaths = ref<AppDataPath[]>([]);
const loadingData = ref(false);
const scanning = ref(false);
const running = ref(false);
const result = ref<CommandOutput | null>(null);
const confirming = ref(false);
const scope = ref<"user" | "system">("user");
const cacheAge = ref<number | null>(appsCacheAge());

const rotatingApp = useRotatingDetail(
  () =>
    scanning.value
      ? [
          "/Applications",
          "/System/Applications",
          "/Applications/Utilities",
          "读取 bundle 信息",
          "统计应用体积",
        ]
      : [],
  700
);

async function load(force = false) {
  const cached = force ? null : peekAppsCache();
  if (cached) {
    apps.value = cached;
    cacheAge.value = appsCacheAge();
    error.value = null;
    return;
  }

  scanning.value = true;
  try {
    apps.value = await loadApps(force);
    cacheAge.value = appsCacheAge();
    error.value = null;
  } catch (e) {
    error.value = String(e);
  } finally {
    scanning.value = false;
  }
}

onMounted(() => {
  load();
});

async function selectApp(app: App) {
  selected.value = app;
  dataPaths.value = [];
  loadingData.value = true;
  try {
    dataPaths.value = await appData(app.bundle_id);
  } catch {
    // 数据目录扫描失败不影响主流程
  } finally {
    loadingData.value = false;
  }
}

async function uninstall() {
  if (!selected.value) return;
  running.value = true;
  result.value = null;
  try {
    const out = await runUninstall(selected.value.name, false);
    result.value = out;
    selected.value = null;
    confirming.value = false;
    invalidateAppsCache();
    await load(true);
  } catch (e) {
    error.value = String(e);
  } finally {
    running.value = false;
  }
}

const inScope = computed(() => {
  return (apps.value ?? []).filter((a) => (scope.value === "system" ? a.protected : !a.protected));
});

const filtered = computed(() => {
  const q = search.value.trim().toLowerCase();
  return inScope.value.filter(
    (a) =>
      q === "" ||
      a.name.toLowerCase().includes(q) ||
      a.bundle_id.toLowerCase().includes(q)
  );
});

const searched = computed(() => {
  const q = search.value.trim().toLowerCase();
  return (apps.value ?? []).filter(
    (a) =>
      q === "" ||
      a.name.toLowerCase().includes(q) ||
      a.bundle_id.toLowerCase().includes(q)
  );
});

const userCount = computed(() => searched.value.filter((a) => !a.protected).length);
const systemCount = computed(() => searched.value.filter((a) => a.protected).length);
</script>
