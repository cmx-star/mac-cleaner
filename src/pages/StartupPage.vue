<template>
  <div class="page">
    <div class="title-row">
      <div class="page-title">{{ t("startup.title") }}</div>
      <button
        class="icon-button"
        :disabled="scanning"
        :title="t('clean.rescan')"
        :aria-label="t('clean.rescan')"
        @click="load"
      >
        <RefreshIcon :spinning="scanning" />
      </button>
    </div>
    <div class="page-subtitle">{{ t("startup.subtitle") }}</div>

    <div v-if="error" class="error-box">{{ error }}</div>

    <Spinner
      v-if="!items && !error"
      :label="rotating ? `${t('startup.scanning')} ${rotating}` : t('startup.scanning')"
    />

    <template v-if="items">
      <div class="toolbar">
        <div class="scope-tabs">
          <button
            :class="['scope-tab', scope === 'all' ? 'active' : '']"
            @click="scope = 'all'"
          >
            {{ t("startup.filter.all") }}
            <span class="scope-count">{{ counts.all }}</span>
          </button>
          <button
            :class="['scope-tab', scope === 'login_item' ? 'active' : '']"
            @click="scope = 'login_item'"
          >
            {{ t("startup.filter.background") }}
            <span class="scope-count">{{ counts.login_item }}</span>
          </button>
          <button
            :class="['scope-tab', scope === 'services' ? 'active' : '']"
            @click="scope = 'services'"
          >
            {{ t("startup.filter.service") }}
            <span class="scope-count">{{ counts.services }}</span>
          </button>
          <button
            :class="['scope-tab', scope === 'user_agent' ? 'active' : '']"
            @click="scope = 'user_agent'"
          >
            {{ t("startup.filter.user_agent") }}
            <span class="scope-count">{{ counts.user_agent }}</span>
          </button>
          <button
            :class="['scope-tab', scope === 'global_agent' ? 'active' : '']"
            @click="scope = 'global_agent'"
          >
            {{ t("startup.filter.global_agent") }}
            <span class="scope-count">{{ counts.global_agent }}</span>
          </button>
          <button
            :class="['scope-tab', scope === 'daemon' ? 'active' : '']"
            @click="scope = 'daemon'"
          >
            {{ t("startup.filter.daemon") }}
            <span class="scope-count">{{ counts.daemon }}</span>
          </button>
        </div>
        <input
          v-model="search"
          class="input"
          :placeholder="t('startup.searchPlaceholder')"
        />
      </div>

      <div v-if="panels.length === 0 || matched.length === 0" class="empty">
        {{ search.trim() === "" ? t("startup.emptyAll") : t("startup.empty") }}
      </div>

      <template v-else>
        <div v-for="panel in panels" :key="panel.key" class="panel">
          <div class="panel-title">
            {{ panel.title }}
            <span class="scope-count" style="margin-left: 8px">
              {{ panel.count }}
            </span>
          </div>
          <div v-for="item in panel.items" :key="item.id" class="row">
            <img
              v-if="item.icon_png_base64"
              class="app-icon"
              :src="`data:image/png;base64,${item.icon_png_base64}`"
              alt=""
              aria-hidden="true"
            />
            <span v-else class="app-icon app-icon-placeholder" aria-hidden="true">
              {{ item.name.slice(0, 1).toUpperCase() }}
            </span>

            <div class="row-main">
              <div class="row-name" style="display: flex; align-items: center; gap: 8px">
                <span>{{ item.name }}</span>
                <span v-if="item.sub_text" class="tag" style="font-size: 11px; padding: 2px 6px">
                  {{ item.sub_text }}
                </span>
                <span v-if="!item.toggleable" class="tag blue" :title="item.reason ?? ''">
                  {{ t("startup.managed") }}
                </span>
              </div>
              <div class="row-path">
                <span v-if="item.developer">
                  <strong>{{ item.developer }}</strong>
                  <span v-if="item.id !== item.name" style="opacity: 0.7; margin-left: 8px">
                    · {{ item.id }}
                  </span>
                </span>
                <template v-else>{{ item.id }}</template>
              </div>
            </div>

            <label
              v-if="item.toggleable"
              class="switch"
              :title="item.enabled ? t('startup.enabled') : t('startup.disabled')"
            >
              <input
                type="checkbox"
                class="switch-input"
                :checked="item.enabled"
                :disabled="busyId === item.id"
                @change="toggle(item)"
              />
              <span class="switch-track" aria-hidden="true">
                <span class="switch-thumb" />
              </span>
              <InlineSpinner v-if="busyId === item.id" />
            </label>
            <button
              v-else
              class="button secondary small"
              :title="item.reason ?? ''"
              @click="openSettings"
            >
              {{ t("startup.openSettings") }}
            </button>
          </div>
        </div>
      </template>
    </template>

    <!-- 底部实体安全留白 -->
    <div class="bottom-spacer" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { openLoginItemsSettings, setStartupItem, startupItems } from "../api";
import { useT } from "../i18n";
import { Spinner, InlineSpinner, useRotatingDetail } from "../components/Loading";
import { RefreshIcon } from "../components/Icons";
import type { StartupItem } from "../types";

type StartupFilterScope = "all" | "login_item" | "services" | "user_agent" | "global_agent" | "daemon";

const t = useT();
const items = ref<StartupItem[] | null>(null);
const error = ref<string | null>(null);
const scanning = ref(false);
const search = ref("");
const scope = ref<StartupFilterScope>("all");
const busyId = ref<string | null>(null);

const rotating = useRotatingDetail(
  () => (scanning.value ? ["读取登录项", "读取后台项", "读取后台服务"] : []),
  700
);

async function load() {
  scanning.value = true;
  try {
    items.value = await startupItems();
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

async function toggle(item: StartupItem) {
  busyId.value = item.id;
  error.value = null;
  try {
    await setStartupItem(item.id, !item.enabled);
    items.value = await startupItems();
  } catch (e) {
    error.value = t("startup.toggleFailed", { message: String(e) });
  } finally {
    busyId.value = null;
  }
}

async function openSettings() {
  try {
    await openLoginItemsSettings();
  } catch (e) {
    error.value = String(e);
  }
}

const counts = computed(() => {
  const all = items.value ?? [];
  const loginItems = all.filter((i) => i.kind === "login_item");
  const userAgents = all.filter((i) => i.kind === "user_agent");
  const globalAgents = all.filter((i) => i.kind === "global_agent");
  const daemons = all.filter((i) => i.kind === "daemon");
  const services = all.filter((i) => i.kind !== "login_item");

  return {
    all: all.length,
    login_item: loginItems.length,
    services: services.length,
    user_agent: userAgents.length,
    global_agent: globalAgents.length,
    daemon: daemons.length,
  };
});

const matched = computed(() => {
  const query = search.value.trim().toLowerCase();
  return (items.value ?? []).filter((i) => {
    const matchesSearch =
      query === "" ||
      i.name.toLowerCase().includes(query) ||
      i.id.toLowerCase().includes(query) ||
      (i.developer ?? "").toLowerCase().includes(query) ||
      (i.sub_text ?? "").toLowerCase().includes(query);

    if (!matchesSearch) return false;

    if (scope.value === "all") return true;
    if (scope.value === "login_item") return i.kind === "login_item";
    if (scope.value === "services") return i.kind !== "login_item";
    return i.kind === scope.value;
  });
});

const panels = computed(() => {
  if (scope.value === "all") {
    const loginItems = matched.value.filter((i) => i.kind === "login_item");
    const serviceItems = matched.value.filter((i) => i.kind !== "login_item");
    const list = [];
    if (loginItems.length > 0) {
      list.push({
        key: "login_item",
        title: t("startup.filter.background"),
        count: loginItems.length,
        items: loginItems,
      });
    }
    if (serviceItems.length > 0) {
      list.push({
        key: "services",
        title: t("startup.filter.service"),
        count: serviceItems.length,
        items: serviceItems,
      });
    }
    return list;
  }

  if (scope.value === "services") {
    const userAgents = matched.value.filter((i) => i.kind === "user_agent");
    const globalAgents = matched.value.filter((i) => i.kind === "global_agent");
    const daemons = matched.value.filter((i) => i.kind === "daemon");
    const list = [];
    if (userAgents.length > 0) {
      list.push({
        key: "user_agent",
        title: t("startup.filter.user_agent"),
        count: userAgents.length,
        items: userAgents,
      });
    }
    if (globalAgents.length > 0) {
      list.push({
        key: "global_agent",
        title: t("startup.filter.global_agent"),
        count: globalAgents.length,
        items: globalAgents,
      });
    }
    if (daemons.length > 0) {
      list.push({
        key: "daemon",
        title: t("startup.filter.daemon"),
        count: daemons.length,
        items: daemons,
      });
    }
    return list;
  }

  const titleMap: Record<StartupFilterScope, string> = {
    all: t("startup.filter.all"),
    login_item: t("startup.filter.background"),
    services: t("startup.filter.service"),
    user_agent: t("startup.filter.user_agent"),
    global_agent: t("startup.filter.global_agent"),
    daemon: t("startup.filter.daemon"),
  };

  return [
    {
      key: scope.value,
      title: titleMap[scope.value],
      count: matched.value.length,
      items: matched.value,
    },
  ];
});
</script>
