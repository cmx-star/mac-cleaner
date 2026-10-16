<template>
  <!-- 初次进入且在扫描中状态 -->
  <div v-if="scanning && !preview" class="page">
    <div class="page-title">{{ t("clean.readyTitle") }}</div>
    <div class="page-subtitle">{{ t("clean.subtitle") }}</div>
    <ScanProgress
      :phases="[
        {
          step: t('clean.scan.step'),
          detail: rotatingDir ? t('clean.scan.detail', { dir: rotatingDir }) : undefined,
        },
      ]"
      :active="0"
    />
  </div>

  <div v-else class="page clean-page-layout">
    <!-- 顶部标题与摘要栏 -->
    <div class="clean-top-bar">
      <div class="clean-title-area">
        <div class="clean-title-row">
          <h1 class="page-title" style="margin: 0">
            {{ t("clean.readyTitle") }}
          </h1>
        </div>
        <div class="clean-summary-sub">
          {{
            t("clean.summaryHeader", {
              size: humanBytes(stats.totalCleanableSize),
              count: stats.totalCleanableCount,
              categories: activeCategoriesCount,
            })
          }}
          <template v-if="stats.topApps.length > 0">
            ·
            <span>
              {{ t("clean.topOccupied", { apps: stats.topApps.join("、") }) }}
            </span>
          </template>
        </div>
      </div>

      <div class="clean-header-actions">
        <button
          class="clean-icon-btn"
          :title="t('clean.rescan')"
          :disabled="scanning || running"
          @click="load(true)"
        >
          <RefreshIcon :spinning="scanning" />
        </button>
      </div>
    </div>

    <div v-if="error" class="error-box" style="margin-bottom: 16px">{{ error }}</div>

    <!-- 清理结果提示横幅 -->
    <div v-if="result" class="clean-result-banner">
      <div>
        <strong>{{ t("clean.result") }}：</strong>
        <span>{{ result.stdout || t("common.noOutput") }}</span>
      </div>
      <button
        class="button secondary"
        style="padding: 4px 10px; font-size: 12px"
        @click="result = null"
      >
        关闭
      </button>
    </div>

    <!-- 分类卡片列表 -->
    <div class="clean-categories-list">
      <template v-for="cat in CATEGORIES" :key="cat.id">
        <div v-if="(categorized.get(cat.id)?.length || 0) > 0" class="clean-category-card">
          <!-- 分类头部行 -->
          <div
            class="clean-category-header"
            @click="toggleCategoryExpand(cat.id)"
          >
            <!-- 分类复选框 -->
            <div
              :class="[
                'custom-checkbox',
                isCategoryAllSelected(cat.id)
                  ? 'checked'
                  : isCategoryIndeterminate(cat.id)
                    ? 'indeterminate'
                    : '',
                getCleanableCount(cat.id) === 0 ? 'disabled' : '',
              ]"
              :title="isCategoryAllSelected(cat.id) ? t('clean.deselectAll') : t('clean.selectAll')"
              @click.stop="toggleCategory(cat.id)"
            >
              <CheckIcon v-if="isCategoryAllSelected(cat.id)" />
              <MinusIcon v-if="isCategoryIndeterminate(cat.id)" />
            </div>

            <!-- 分类信息 -->
            <div class="clean-category-info">
              <div class="clean-category-title-row">
                <span class="clean-category-name">{{ t(cat.nameKey) }}</span>
                <span class="clean-category-count-badge">
                  {{
                    t("clean.selectedRatio", {
                      selected: getSelectedCount(cat.id),
                      total: categorized.get(cat.id)?.length || 0,
                    })
                  }}
                </span>
              </div>
              <div class="clean-category-desc">{{ t(cat.descKey) }}</div>
            </div>

            <!-- 右侧体积与展开箭头 -->
            <div class="clean-category-meta">
              <div class="clean-category-sizes">
                <span>{{ humanBytes(getCategorySelectedSize(cat.id)) }}</span>
                <span class="clean-category-sizes-dim"> / {{ humanBytes(getCategoryTotalSize(cat.id)) }}</span>
              </div>
              <span class="clean-category-chevron">
                <ChevronDownIcon :open="expandedCategories.has(cat.id)" />
              </span>
            </div>
          </div>

          <!-- 展开的明细项列表 -->
          <div v-if="expandedCategories.has(cat.id)" class="clean-items-container">
            <div
              v-for="item in categorized.get(cat.id)"
              :key="item.path"
              class="clean-item-row"
              :style="{ cursor: isItemCleanable(item) ? 'pointer' : 'default' }"
              @click="toggleItem(item.path, isItemCleanable(item))"
            >
              <!-- 单项复选框 -->
              <div
                :class="[
                  'custom-checkbox',
                  selectedPaths.has(item.path) ? 'checked' : '',
                  !isItemCleanable(item) ? 'disabled' : '',
                ]"
                @click.stop="toggleItem(item.path, isItemCleanable(item))"
              >
                <CheckIcon v-if="selectedPaths.has(item.path)" />
              </div>

              <!-- 单项主体信息 -->
              <div class="clean-item-main">
                <div class="clean-item-header">
                  <span class="clean-item-name">
                    {{ item.display_name || item.name }}
                  </span>
                  <span :class="['tag', TIER_CLASSES[item.tier]]" style="font-size: 10px">
                    {{ t(TIER_KEYS[item.tier]) }}
                  </span>
                  <span v-if="item.protection_reason" class="tag blue" style="font-size: 10px">
                    {{ item.protection_reason }}
                  </span>
                </div>
                <div class="clean-item-detail" :title="item.path">
                  {{ item.description ? `${item.description} · ` : "" }}{{ item.path }}
                </div>
              </div>

              <!-- 单项体积 -->
              <div class="clean-item-size">{{ humanBytes(item.size) }}</div>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- 底部悬浮/固定操作条 -->
    <div class="clean-sticky-footer">
      <div class="clean-footer-left">
        <span>
          {{ t("clean.selectedCount", { count: stats.selectedCount }) }}
        </span>
        <span>·</span>
        <button
          type="button"
          class="clean-action-link"
          @click="handleSelectAll"
        >
          {{ t("clean.selectAll") }}
        </button>
        <span>·</span>
        <button
          type="button"
          class="clean-action-link"
          @click="handleDeselectAll"
        >
          {{ t("clean.deselectAll") }}
        </button>
        <span>·</span>
        <button
          type="button"
          class="clean-action-link active"
          @click="handleRecommend"
        >
          {{ t("clean.recommend") }}
        </button>
      </div>

      <div class="clean-footer-right">
        <div v-if="running && runningDetail" style="display: flex; align-items: center; gap: 6px; color: var(--text-dim); font-size: 12px">
          <InlineSpinner />
          <span>{{ runningDetail }}</span>
        </div>

        <button
          class="clean-submit-button"
          :disabled="stats.selectedCount === 0 || running"
          @click="executeClean"
        >
          <InlineSpinner v-if="running" />
          <span>
            {{ t("clean.actionPermanent", { size: humanBytes(stats.selectedSize) }) }}
          </span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { cleanupPreview, humanBytes, runClean } from "../api";
import { useT } from "../i18n";
import { InlineSpinner, ScanProgress, useRotatingDetail } from "../components/Loading";
import { CheckIcon, ChevronDownIcon, MinusIcon, RefreshIcon } from "../components/Icons";
import type { CleanupItem, CleanupPreview, CommandOutput, Tier } from "../types";

const TIER_KEYS: Record<Tier, "clean.tier.regenerable" | "clean.tier.expensive" | "clean.tier.irreplaceable"> = {
  regenerable: "clean.tier.regenerable",
  expensive: "clean.tier.expensive",
  irreplaceable: "clean.tier.irreplaceable",
};

const TIER_CLASSES: Record<Tier, string> = {
  regenerable: "green",
  expensive: "yellow",
  irreplaceable: "red",
};

interface CategoryDef {
  id: "app" | "communication" | "developer" | "system";
  nameKey: "clean.category.app" | "clean.category.communication" | "clean.category.developer" | "clean.category.system";
  descKey: "clean.category.appDesc" | "clean.category.communicationDesc" | "clean.category.developerDesc" | "clean.category.systemDesc";
}

const CATEGORIES: CategoryDef[] = [
  {
    id: "app",
    nameKey: "clean.category.app",
    descKey: "clean.category.appDesc",
  },
  {
    id: "communication",
    nameKey: "clean.category.communication",
    descKey: "clean.category.communicationDesc",
  },
  {
    id: "developer",
    nameKey: "clean.category.developer",
    descKey: "clean.category.developerDesc",
  },
  {
    id: "system",
    nameKey: "clean.category.system",
    descKey: "clean.category.systemDesc",
  },
];

// 模块级缓存
let cachedPreview: CleanupPreview | null = null;
let cachedSelectedPaths: Set<string> | null = null;
let cachedExpandedCategories: Set<string> | null = null;

const t = useT();
const preview = ref<CleanupPreview | null>(cachedPreview);
const selectedPaths = ref<Set<string>>(cachedSelectedPaths ? new Set(cachedSelectedPaths) : new Set());
const expandedCategories = ref<Set<string>>(cachedExpandedCategories ? new Set(cachedExpandedCategories) : new Set());
const error = ref<string | null>(null);
const scanning = ref(false);
const running = ref(false);
const runningDetail = ref<string | null>(null);
const result = ref<CommandOutput | null>(null);

const watchDirs = [
  "~/Library/Caches",
  "~/.cache",
  "~/Library/Logs",
  "~/Library/Developer/Xcode/DerivedData",
  "~/.npm",
  "~/Library/Caches/Homebrew",
];
const rotatingDir = useRotatingDetail(() => (scanning.value ? watchDirs : []));

function isItemCleanable(item: CleanupItem) {
  return (
    item.tier === "regenerable" &&
    !item.protection_reason &&
    item.protection !== "system" &&
    item.protection !== "whitelisted"
  );
}

watch(selectedPaths, (val) => {
  cachedSelectedPaths = val;
});

watch(expandedCategories, (val) => {
  cachedExpandedCategories = val;
});

async function load(force = false) {
  if (!force && cachedPreview) {
    preview.value = cachedPreview;
    if (cachedSelectedPaths) {
      selectedPaths.value = new Set(cachedSelectedPaths);
    }
    return;
  }

  scanning.value = true;
  result.value = null;
  try {
    const data = await cleanupPreview(force);
    cachedPreview = data;
    preview.value = data;
    error.value = null;

    const recommendedSet = new Set<string>();
    for (const item of data.items) {
      if (item.recommended && isItemCleanable(item)) {
        recommendedSet.add(item.path);
      }
    }
    cachedSelectedPaths = recommendedSet;
    selectedPaths.value = recommendedSet;
  } catch (e) {
    error.value = String(e);
  } finally {
    scanning.value = false;
  }
}

onMounted(() => {
  if (!cachedPreview) {
    load(false);
  }
});

const categorized = computed(() => {
  const map = new Map<string, CleanupItem[]>();
  for (const cat of CATEGORIES) {
    map.set(cat.id, []);
  }

  if (!preview.value) return map;

  for (const item of preview.value.items) {
    const catId = item.category && map.has(item.category) ? item.category : "app";
    map.get(catId)?.push(item);
  }

  return map;
});

const stats = computed(() => {
  if (!preview.value) {
    return {
      totalCleanableCount: 0,
      totalCleanableSize: 0,
      selectedCount: 0,
      selectedSize: 0,
      topApps: [] as string[],
    };
  }

  let totalCleanableCount = 0;
  let totalCleanableSize = 0;
  let selectedCount = 0;
  let selectedSize = 0;

  for (const item of preview.value.items) {
    if (isItemCleanable(item)) {
      totalCleanableCount += 1;
      totalCleanableSize += item.size;
      if (selectedPaths.value.has(item.path)) {
        selectedCount += 1;
        selectedSize += item.size;
      }
    }
  }

  const topApps = preview.value.items
    .filter((i) => i.size > 20 * 1024 * 1024)
    .slice(0, 2)
    .map((i) => i.display_name || i.name);

  return {
    totalCleanableCount,
    totalCleanableSize,
    selectedCount,
    selectedSize,
    topApps,
  };
});

const activeCategoriesCount = computed(() => {
  return CATEGORIES.filter((c) => (categorized.value.get(c.id)?.length || 0) > 0).length;
});

function getCleanableCount(catId: string) {
  const items = categorized.value.get(catId) || [];
  return items.filter(isItemCleanable).length;
}

function getSelectedCount(catId: string) {
  const items = categorized.value.get(catId) || [];
  return items.filter(isItemCleanable).filter((i) => selectedPaths.value.has(i.path)).length;
}

function isCategoryAllSelected(catId: string) {
  const items = categorized.value.get(catId) || [];
  const cleanable = items.filter(isItemCleanable);
  return cleanable.length > 0 && cleanable.every((i) => selectedPaths.value.has(i.path));
}

function isCategoryIndeterminate(catId: string) {
  const items = categorized.value.get(catId) || [];
  const cleanable = items.filter(isItemCleanable);
  const selectedCount = cleanable.filter((i) => selectedPaths.value.has(i.path)).length;
  return selectedCount > 0 && selectedCount < cleanable.length;
}

function getCategoryTotalSize(catId: string) {
  const items = categorized.value.get(catId) || [];
  return items.reduce((acc, i) => acc + i.size, 0);
}

function getCategorySelectedSize(catId: string) {
  const items = categorized.value.get(catId) || [];
  return items
    .filter(isItemCleanable)
    .filter((i) => selectedPaths.value.has(i.path))
    .reduce((acc, i) => acc + i.size, 0);
}

function toggleCategoryExpand(catId: string) {
  const next = new Set(expandedCategories.value);
  if (next.has(catId)) {
    next.delete(catId);
  } else {
    next.add(catId);
  }
  expandedCategories.value = next;
}

function toggleItem(path: string, cleanable: boolean) {
  if (!cleanable) return;
  const next = new Set(selectedPaths.value);
  if (next.has(path)) {
    next.delete(path);
  } else {
    next.add(path);
  }
  selectedPaths.value = next;
}

function toggleCategory(catId: string) {
  const items = categorized.value.get(catId) || [];
  const cleanable = items.filter(isItemCleanable);
  if (cleanable.length === 0) return;

  const allSelected = cleanable.every((i) => selectedPaths.value.has(i.path));
  const next = new Set(selectedPaths.value);

  if (allSelected) {
    for (const item of cleanable) {
      next.delete(item.path);
    }
  } else {
    for (const item of cleanable) {
      next.add(item.path);
    }
  }
  selectedPaths.value = next;
}

function handleSelectAll() {
  if (!preview.value) return;
  const all = new Set<string>();
  for (const item of preview.value.items) {
    if (isItemCleanable(item)) {
      all.add(item.path);
    }
  }
  selectedPaths.value = all;
}

function handleDeselectAll() {
  selectedPaths.value = new Set();
}

function handleRecommend() {
  if (!preview.value) return;
  const rec = new Set<string>();
  for (const item of preview.value.items) {
    if (item.recommended && isItemCleanable(item)) {
      rec.add(item.path);
    }
  }
  selectedPaths.value = rec;
}

async function executeClean() {
  if (selectedPaths.value.size === 0) return;

  running.value = true;
  runningDetail.value = t("clean.deleting", { name: "选定项目" });
  result.value = null;

  try {
    const pathsToClean = Array.from(selectedPaths.value);
    const res = await runClean(false, pathsToClean);
    result.value = res;
    cachedPreview = null;
    cachedSelectedPaths = null;
    await load(true);
  } catch (e) {
    error.value = String(e);
  } finally {
    running.value = false;
    runningDetail.value = null;
  }
}
</script>
