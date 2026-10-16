import { useCallback, useEffect, useState } from "react";
import { listOptimizeTasks, runOptimizeTasks } from "../api";
import { useT, type MessageKey } from "../i18n";
import { Spinner, InlineSpinner } from "../components/Loading";
import { RefreshIcon } from "../components/Icons";
import type { OptimizeTaskDef, OptimizeTaskResult } from "../types";

/** 优化任务对应的内置图标渲染 */
function TaskIcon({ name }: { name: string }) {
  switch (name) {
    case "globe":
      return (
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="10" />
          <line x1="2" y1="12" x2="22" y2="12" />
          <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
        </svg>
      );
    case "trash":
      return (
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="3 6 5 6 21 6" />
          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
        </svg>
      );
    case "eye":
      return (
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
          <circle cx="12" cy="12" r="3" />
        </svg>
      );
    case "grid":
      return (
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <rect x="3" y="3" width="7" height="7" />
          <rect x="14" y="3" width="7" height="7" />
          <rect x="14" y="14" width="7" height="7" />
          <rect x="3" y="14" width="7" height="7" />
        </svg>
      );
    case "clipboard":
      return (
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
          <rect x="8" y="2" width="8" height="4" rx="1" ry="1" />
        </svg>
      );
    case "type":
      return (
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="4 7 4 4 20 4 20 7" />
          <line x1="9" y1="20" x2="15" y2="20" />
          <line x1="12" y1="4" x2="12" y2="20" />
        </svg>
      );
    default:
      return <span>⚡</span>;
  }
}

export default function OptimizePage() {
  const [tasks, setTasks] = useState<OptimizeTaskDef[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [runningTaskId, setRunningTaskId] = useState<string | null>(null);
  const [runningAll, setRunningAll] = useState(false);
  const [results, setResults] = useState<Record<string, OptimizeTaskResult>>({});
  const [lastBatchMessage, setLastBatchMessage] = useState<string | null>(null);
  const t = useT();

  const loadTasks = useCallback(async () => {
    setLoading(true);
    try {
      const list = await listOptimizeTasks();
      setTasks(list);
      // 默认选中推荐项
      const rec = new Set(list.filter((x) => x.recommended).map((x) => x.id));
      setSelectedIds(rec);
    } catch (e) {
      console.error("加载优化任务定义失败", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadTasks();
  }, [loadTasks]);

  const toggleSelect = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const selectAll = () => {
    setSelectedIds(new Set(tasks.map((t) => t.id)));
  };

  const selectNone = () => {
    setSelectedIds(new Set());
  };

  const selectRecommended = () => {
    setSelectedIds(new Set(tasks.filter((t) => t.recommended).map((t) => t.id)));
  };

  // 执行单项
  const runSingle = async (id: string) => {
    if (runningTaskId || runningAll) return;
    setRunningTaskId(id);
    setLastBatchMessage(null);
    try {
      const res = await runOptimizeTasks([id]);
      if (res && res.length > 0) {
        setResults((prev) => ({ ...prev, [id]: res[0] }));
      }
    } catch (e) {
      setResults((prev) => ({
        ...prev,
        [id]: { id, success: false, message: String(e) },
      }));
    } finally {
      setRunningTaskId(null);
    }
  };

  // 批量执行选中的任务
  const runSelected = async () => {
    if (selectedIds.size === 0 || runningAll || runningTaskId) return;
    setRunningAll(true);
    setLastBatchMessage(null);
    const targetIds = Array.from(selectedIds);
    try {
      const taskResults = await runOptimizeTasks(targetIds);
      const newMap: Record<string, OptimizeTaskResult> = {};
      let successCount = 0;
      for (const r of taskResults) {
        newMap[r.id] = r;
        if (r.success) successCount++;
      }
      setResults((prev) => ({ ...prev, ...newMap }));
      setLastBatchMessage(t("optimize.successCount", { count: successCount }));
    } catch (e) {
      setLastBatchMessage(`执行失败：${String(e)}`);
    } finally {
      setRunningAll(false);
    }
  };

  if (loading) {
    return (
      <div className="page center">
        <Spinner />
      </div>
    );
  }

  return (
    <div className="page" style={{ paddingBottom: 90 }}>
      <div className="title-row">
        <div>
          <div className="page-title">{t("optimize.title")}</div>
          <div className="page-desc" style={{ marginTop: 4, color: "var(--text-dim)", fontSize: 13 }}>
            {t("optimize.subtitle")}
          </div>
        </div>
        <button
          className="icon-button"
          onClick={loadTasks}
          disabled={runningAll || runningTaskId !== null}
          title={t("common.refresh")}
        >
          <RefreshIcon />
        </button>
      </div>

      {lastBatchMessage && (
        <div className="clean-result-banner" style={{ marginTop: 12 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <span style={{ color: "var(--green)", fontSize: 16 }}>✓</span>
            <span style={{ fontSize: 13, fontWeight: 500 }}>{lastBatchMessage}</span>
          </div>
          <button
            className="text-button"
            onClick={() => setLastBatchMessage(null)}
            style={{ fontSize: 12, color: "var(--text-dim)", cursor: "pointer", background: "none", border: "none" }}
          >
            ✕
          </button>
        </div>
      )}

      <div style={{ display: "flex", flexDirection: "column", gap: 12, marginTop: 18 }}>
        {tasks.map((task) => {
          const isChecked = selectedIds.has(task.id);
          const isRunning = runningTaskId === task.id || (runningAll && selectedIds.has(task.id));
          const result = results[task.id];

          return (
            <div
              key={task.id}
              className={`optimize-card ${isChecked ? "checked" : ""}`}
              style={{
                background: "var(--bg-panel)",
                border: isChecked ? "1px solid rgba(91, 163, 245, 0.45)" : "1px solid var(--border)",
                borderRadius: "var(--radius)",
                padding: "16px 20px",
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                gap: 16,
                transition: "all 0.15s ease",
              }}
            >
              {/* 左侧勾选与图标 */}
              <div style={{ display: "flex", alignItems: "center", gap: 16, flex: 1, minWidth: 0 }}>
                <input
                  type="checkbox"
                  checked={isChecked}
                  onChange={() => toggleSelect(task.id)}
                  disabled={runningAll || runningTaskId !== null}
                  style={{
                    width: 17,
                    height: 17,
                    accentColor: "var(--accent)",
                    cursor: "pointer",
                  }}
                />

                <div
                  style={{
                    width: 42,
                    height: 42,
                    borderRadius: 10,
                    background: isChecked ? "rgba(91, 163, 245, 0.15)" : "rgba(255, 255, 255, 0.05)",
                    color: isChecked ? "var(--accent)" : "var(--text-dim)",
                    display: "grid",
                    placeItems: "center",
                    flexShrink: 0,
                  }}
                >
                  <TaskIcon name={task.icon_name} />
                </div>

                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                    <span style={{ fontSize: 14, fontWeight: 600, color: "var(--text)" }}>
                      {t(task.name_key as MessageKey)}
                    </span>
                    {task.recommended && (
                      <span
                        style={{
                          fontSize: 10,
                          padding: "1px 6px",
                          borderRadius: 4,
                          background: "rgba(76, 195, 138, 0.15)",
                          color: "var(--green)",
                          fontWeight: 500,
                        }}
                      >
                        {t("optimize.recommended")}
                      </span>
                    )}
                  </div>
                  <div style={{ fontSize: 12, color: "var(--text-dim)", lineHeight: 1.45 }}>
                    {t(task.desc_key as MessageKey)}
                  </div>

                  {result && (
                    <div
                      style={{
                        marginTop: 6,
                        fontSize: 12,
                        display: "flex",
                        alignItems: "center",
                        gap: 6,
                        color: result.success ? "var(--green)" : "var(--red)",
                      }}
                    >
                      <span>{result.success ? "✓" : "✕"}</span>
                      <span>{result.message}</span>
                    </div>
                  )}
                </div>
              </div>

              {/* 右侧单个操作按钮 */}
              <div>
                <button
                  className="secondary-button"
                  onClick={() => runSingle(task.id)}
                  disabled={runningAll || runningTaskId !== null}
                  style={{
                    padding: "6px 14px",
                    borderRadius: 8,
                    fontSize: 12,
                    fontWeight: 500,
                    cursor: "pointer",
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                    background: "rgba(255, 255, 255, 0.06)",
                    border: "1px solid var(--border)",
                    color: "var(--text)",
                  }}
                >
                  {isRunning ? <InlineSpinner /> : null}
                  {isRunning ? t("optimize.running") : t("optimize.runSingle")}
                </button>
              </div>
            </div>
          );
        })}
      </div>

      {/* 底部悬浮控制栏 */}
      <div className="clean-sticky-footer">
        <div className="clean-footer-left">
          <span>
            {t("clean.selectedCount", { count: selectedIds.size })}
          </span>
          <span>·</span>
          <button
            type="button"
            className={`clean-action-link ${tasks.length > 0 && selectedIds.size === tasks.length ? "active" : ""}`}
            onClick={selectAll}
            disabled={runningAll || runningTaskId !== null}
          >
            {t("optimize.all")}
          </button>
          <span>·</span>
          <button
            type="button"
            className={`clean-action-link ${selectedIds.size === 0 ? "active" : ""}`}
            onClick={selectNone}
            disabled={runningAll || runningTaskId !== null}
          >
            {t("optimize.none")}
          </button>
          <span>·</span>
          <button
            type="button"
            className={`clean-action-link ${
              tasks.length > 0 &&
              tasks.filter((t) => t.recommended).every((t) => selectedIds.has(t.id)) &&
              selectedIds.size === tasks.filter((t) => t.recommended).length
                ? "active"
                : ""
            }`}
            onClick={selectRecommended}
            disabled={runningAll || runningTaskId !== null}
          >
            {t("optimize.recommended")}
          </button>
        </div>

        <div className="clean-footer-right">
          <button
            type="button"
            className="clean-submit-button"
            onClick={runSelected}
            disabled={selectedIds.size === 0 || runningAll || runningTaskId !== null}
          >
            {runningAll ? <InlineSpinner /> : <span>⚡</span>}
            <span>
              {runningAll
                ? t("optimize.running")
                : t("optimize.runSelected", { count: selectedIds.size })}
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}
