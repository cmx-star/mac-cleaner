import { useCallback, useEffect, useMemo, useState } from "react";
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

export default function CleanPage() {
  const [preview, setPreview] = useState<CleanupPreview | null>(null);
  const [selectedPaths, setSelectedPaths] = useState<Set<string>>(new Set());
  const [expandedCategories, setExpandedCategories] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [running, setRunning] = useState(false);
  const [runningDetail, setRunningDetail] = useState<string | null>(null);
  const [result, setResult] = useState<CommandOutput | null>(null);
  const t = useT();

  // 扫描中轮播展示常见缓存目录，提供良好的等待反馈
  const watchDirs = [
    "~/Library/Caches",
    "~/.cache",
    "~/Library/Logs",
    "~/Library/Developer/Xcode/DerivedData",
    "~/.npm",
    "~/Library/Caches/Homebrew",
  ];
  const rotatingDir = useRotatingDetail(scanning ? watchDirs : []);

  // 判定项目是否可安全选择进行清理
  const isItemCleanable = useCallback((item: CleanupItem) => {
    return item.tier === "regenerable" && !item.protection_reason && item.protection !== "system" && item.protection !== "whitelisted";
  }, []);

  const load = useCallback(async () => {
    setScanning(true);
    setResult(null);
    try {
      const data = await cleanupPreview();
      setPreview(data);
      setError(null);

      // 默认推荐选中的项目：标记为 recommended 且可安全再生的项
      const recommendedSet = new Set<string>();
      for (const item of data.items) {
        if (item.recommended && isItemCleanable(item)) {
          recommendedSet.add(item.path);
        }
      }
      setSelectedPaths(recommendedSet);
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }, [isItemCleanable]);

  // 组件加载时自动开始扫描
  useEffect(() => {
    load();
  }, [load]);

  // 按分类对缓存项目归组
  const categorized = useMemo(() => {
    const map = new Map<string, CleanupItem[]>();
    for (const cat of CATEGORIES) {
      map.set(cat.id, []);
    }

    if (!preview) return map;

    for (const item of preview.items) {
      const catId = item.category && map.has(item.category) ? item.category : "app";
      map.get(catId)?.push(item);
    }

    return map;
  }, [preview]);

  // 计算整体可清理与已选中的统计信息
  const stats = useMemo(() => {
    if (!preview) {
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

    for (const item of preview.items) {
      if (isItemCleanable(item)) {
        totalCleanableCount += 1;
        totalCleanableSize += item.size;
        if (selectedPaths.has(item.path)) {
          selectedCount += 1;
          selectedSize += item.size;
        }
      }
    }

    // 找出占用缓存最多的前 2 个大型应用或组件
    const topApps = preview.items
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
  }, [preview, selectedPaths, isItemCleanable]);

  // 展开或收起指定分类
  const toggleCategoryExpand = (catId: string) => {
    setExpandedCategories((prev) => {
      const next = new Set(prev);
      if (next.has(catId)) {
        next.delete(catId);
      } else {
        next.add(catId);
      }
      return next;
    });
  };

  // 切换单个项目的勾选状态
  const toggleItem = (path: string, cleanable: boolean) => {
    if (!cleanable) return;
    setSelectedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  };

  // 切换某分类下全部项目的勾选状态
  const toggleCategory = (catId: string) => {
    const items = categorized.get(catId) || [];
    const cleanable = items.filter(isItemCleanable);
    if (cleanable.length === 0) return;

    const allSelected = cleanable.every((i) => selectedPaths.has(i.path));

    setSelectedPaths((prev) => {
      const next = new Set(prev);
      if (allSelected) {
        // 全不选该分类
        for (const item of cleanable) {
          next.delete(item.path);
        }
      } else {
        // 全选该分类
        for (const item of cleanable) {
          next.add(item.path);
        }
      }
      return next;
    });
  };

  // 快捷操作：全选
  const handleSelectAll = () => {
    if (!preview) return;
    const all = new Set<string>();
    for (const item of preview.items) {
      if (isItemCleanable(item)) {
        all.add(item.path);
      }
    }
    setSelectedPaths(all);
  };

  // 快捷操作：全不选
  const handleDeselectAll = () => {
    setSelectedPaths(new Set());
  };

  // 快捷操作：推荐选法
  const handleRecommend = () => {
    if (!preview) return;
    const rec = new Set<string>();
    for (const item of preview.items) {
      if (item.recommended && isItemCleanable(item)) {
        rec.add(item.path);
      }
    }
    setSelectedPaths(rec);
  };

  // 执行清理所选项目
  const executeClean = async () => {
    if (selectedPaths.size === 0) return;

    setRunning(true);
    setRunningDetail(t("clean.deleting", { name: "选定项目" }));
    setResult(null);

    try {
      const pathsToClean = Array.from(selectedPaths);
      const res = await runClean(false, pathsToClean);
      setResult(res);
      // 清理完毕后刷新扫描结果
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
      setRunningDetail(null);
    }
  };

  // 初次进入且在扫描中状态
  if (scanning && !preview) {
    return (
      <div className="page">
        <div className="page-title">{t("clean.readyTitle")}</div>
        <div className="page-subtitle">{t("clean.subtitle")}</div>
        <ScanProgress
          phases={[
            {
              step: t("clean.scan.step"),
              detail: rotatingDir ? t("clean.scan.detail", { dir: rotatingDir }) : undefined,
            },
          ]}
          active={0}
        />
      </div>
    );
  }

  const activeCategoriesCount = CATEGORIES.filter(
    (c) => (categorized.get(c.id)?.length || 0) > 0,
  ).length;

  return (
    <div className="page clean-page-layout">
      {/* 顶部标题与摘要栏 */}
      <div className="clean-top-bar">
        <div className="clean-title-area">
          <div className="clean-title-row">
            <h1 className="page-title" style={{ margin: 0 }}>
              {t("clean.readyTitle")}
            </h1>
          </div>
          <div className="clean-summary-sub">
            {t("clean.summaryHeader", {
              size: humanBytes(stats.totalCleanableSize),
              count: stats.totalCleanableCount,
              categories: activeCategoriesCount,
            })}
            {stats.topApps.length > 0 && (
              <>
                {" · "}
                <span>
                  {t("clean.topOccupied", { apps: stats.topApps.join("、") })}
                </span>
              </>
            )}
          </div>
        </div>

        <div className="clean-header-actions">
          <button
            className="clean-icon-btn"
            title={t("clean.rescan")}
            onClick={load}
            disabled={scanning || running}
          >
            <RefreshIcon spinning={scanning} />
          </button>
        </div>
      </div>

      {error && <div className="error-box" style={{ marginBottom: 16 }}>{error}</div>}

      {/* 清理结果提示横幅 */}
      {result && (
        <div className="clean-result-banner">
          <div>
            <strong>{t("clean.result")}：</strong>
            <span>{result.stdout || t("common.noOutput")}</span>
          </div>
          <button
            className="button secondary"
            style={{ padding: "4px 10px", fontSize: 12 }}
            onClick={() => setResult(null)}
          >
            关闭
          </button>
        </div>
      )}

      {/* 分类卡片列表 */}
      <div className="clean-categories-list">
        {CATEGORIES.map((cat) => {
          const items = categorized.get(cat.id) || [];
          if (items.length === 0) return null;

          const cleanableItems = items.filter(isItemCleanable);
          const selectedInCat = cleanableItems.filter((i) => selectedPaths.has(i.path));
          const catTotalSize = items.reduce((acc, i) => acc + i.size, 0);
          const catSelectedSize = selectedInCat.reduce((acc, i) => acc + i.size, 0);

          const isAllSelected = cleanableItems.length > 0 && selectedInCat.length === cleanableItems.length;
          const isIndeterminate = selectedInCat.length > 0 && selectedInCat.length < cleanableItems.length;
          const isExpanded = expandedCategories.has(cat.id);

          return (
            <div key={cat.id} className="clean-category-card">
              {/* 分类头部行 */}
              <div
                className="clean-category-header"
                onClick={() => toggleCategoryExpand(cat.id)}
              >
                {/* 分类复选框 */}
                <div
                  className={`custom-checkbox ${
                    isAllSelected ? "checked" : isIndeterminate ? "indeterminate" : ""
                  } ${cleanableItems.length === 0 ? "disabled" : ""}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    toggleCategory(cat.id);
                  }}
                  title={isAllSelected ? t("clean.deselectAll") : t("clean.selectAll")}
                >
                  {isAllSelected && <CheckIcon />}
                  {isIndeterminate && <MinusIcon />}
                </div>

                {/* 分类信息 */}
                <div className="clean-category-info">
                  <div className="clean-category-title-row">
                    <span className="clean-category-name">{t(cat.nameKey)}</span>
                    <span className="clean-category-count-badge">
                      {t("clean.selectedRatio", {
                        selected: selectedInCat.length,
                        total: items.length,
                      })}
                    </span>
                  </div>
                  <div className="clean-category-desc">{t(cat.descKey)}</div>
                </div>

                {/* 右侧体积与展开箭头 */}
                <div className="clean-category-meta">
                  <div className="clean-category-sizes">
                    <span>{humanBytes(catSelectedSize)}</span>
                    <span className="clean-category-sizes-dim"> / {humanBytes(catTotalSize)}</span>
                  </div>
                  <span className="clean-category-chevron">
                    <ChevronDownIcon open={isExpanded} />
                  </span>
                </div>
              </div>

              {/* 展开的明细项列表 */}
              {isExpanded && (
                <div className="clean-items-container">
                  {items.map((item) => {
                    const cleanable = isItemCleanable(item);
                    const checked = selectedPaths.has(item.path);

                    return (
                      <div
                        key={item.path}
                        className="clean-item-row"
                        onClick={() => toggleItem(item.path, cleanable)}
                        style={{ cursor: cleanable ? "pointer" : "default" }}
                      >
                        {/* 单项复选框 */}
                        <div
                          className={`custom-checkbox ${checked ? "checked" : ""} ${
                            !cleanable ? "disabled" : ""
                          }`}
                          onClick={(e) => {
                            e.stopPropagation();
                            toggleItem(item.path, cleanable);
                          }}
                        >
                          {checked && <CheckIcon />}
                        </div>

                        {/* 单项主体信息 */}
                        <div className="clean-item-main">
                          <div className="clean-item-header">
                            <span className="clean-item-name">
                              {item.display_name || item.name}
                            </span>
                            <span className={`tag ${TIER_CLASSES[item.tier]}`} style={{ fontSize: 10 }}>
                              {t(TIER_KEYS[item.tier])}
                            </span>
                            {item.protection_reason && (
                              <span className="tag blue" style={{ fontSize: 10 }}>
                                {item.protection_reason}
                              </span>
                            )}
                          </div>
                          <div className="clean-item-detail" title={item.path}>
                            {item.description ? `${item.description} · ` : ""}
                            {item.path}
                          </div>
                        </div>

                        {/* 单项体积 */}
                        <div className="clean-item-size">{humanBytes(item.size)}</div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* 底部悬浮/固定操作条 */}
      <div className="clean-sticky-footer">
        <div className="clean-footer-left">
          <span>
            {t("clean.selectedCount", { count: stats.selectedCount })}
          </span>
          <span>·</span>
          <button
            type="button"
            className="clean-action-link"
            onClick={handleSelectAll}
          >
            {t("clean.selectAll")}
          </button>
          <span>·</span>
          <button
            type="button"
            className="clean-action-link"
            onClick={handleDeselectAll}
          >
            {t("clean.deselectAll")}
          </button>
          <span>·</span>
          <button
            type="button"
            className="clean-action-link active"
            onClick={handleRecommend}
          >
            {t("clean.recommend")}
          </button>
        </div>

        <div className="clean-footer-right">
          {running && runningDetail && (
            <div style={{ display: "flex", alignItems: "center", gap: 6, color: "var(--text-dim)", fontSize: 12 }}>
              <InlineSpinner />
              <span>{runningDetail}</span>
            </div>
          )}

          <button
            className="clean-submit-button"
            disabled={stats.selectedCount === 0 || running}
            onClick={executeClean}
          >
            {running && <InlineSpinner />}
            <span>
              {t("clean.actionPermanent", { size: humanBytes(stats.selectedSize) })}
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}
