import { useCallback, useEffect, useState } from "react";
import { readHistory } from "../api";
import type { History } from "../types";

const ACTION_LABELS: Record<string, string> = {
  removed: "删除",
  trashed: "废纸篓",
  skipped: "跳过",
  failed: "失败",
};

export default function HistoryPage() {
  const [history, setHistory] = useState<History | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setHistory(await readHistory(50));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  if (error) return <div className="error-box">{error}</div>;
  if (!history) return <div className="loading pulse">正在读取历史…</div>;

  const hasContent = history.sessions.length > 0 || history.deletions.length > 0;

  return (
    <div className="page">
      <div className="page-title">历史</div>
      <div className="page-subtitle">
        来自 Mole 的操作日志，记录每一次清理与删除的痕迹。
      </div>

      {!hasContent ? (
        <div className="empty">
          还没有操作记录。执行一次清理或卸载后，这里会显示发生了什么。
        </div>
      ) : (
        <>
          {history.sessions.length > 0 && (
            <div className="panel">
              <div className="panel-title">清理会话</div>
              {history.sessions.map((s, i) => (
                <div key={i} className="row">
                  <div className="row-main">
                    <div className="row-name">
                      {s.command}
                      <span className="tag blue">{s.items} 项</span>
                      {s.failed > 0 && <span className="tag red">{s.failed} 失败</span>}
                    </div>
                    <div className="row-path">
                      {s.started_at}
                      {s.ended_at !== s.started_at && ` → ${s.ended_at}`}
                    </div>
                  </div>
                  <div className="row-size">{s.size}</div>
                </div>
              ))}
            </div>
          )}

          {history.deletions.length > 0 && (
            <div className="panel">
              <div className="panel-title">删除记录</div>
              {history.deletions.map((d, i) => (
                <div key={i} className="row">
                  <div className="row-main">
                    <div className="row-path" style={{ fontSize: 12 }}>{d.path || d.timestamp}</div>
                    <div className="row-path">
                      {d.timestamp} · {d.mode}
                      {d.status !== "ok" && ` · ${d.status}`}
                    </div>
                  </div>
                  <div className="row-size">{d.size_display}</div>
                </div>
              ))}
            </div>
          )}
        </>
      )}

      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <button className="button secondary" onClick={load}>
          刷新
        </button>
      </div>
      <div style={{ color: "var(--text-dim)", fontSize: 11, marginTop: 8 }}>
        日志位置：{history.operations_log}
      </div>
    </div>
  );
}
