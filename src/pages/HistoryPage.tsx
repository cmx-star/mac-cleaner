import { useCallback, useEffect, useState } from "react";
import { readHistory } from "../api";
import { useT } from "../i18n";
import { Spinner } from "../components/Loading";
import type { History } from "../types";

export default function HistoryPage() {
  const [history, setHistory] = useState<History | null>(null);
  const [error, setError] = useState<string | null>(null);
  const t = useT();

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
  if (!history) return <Spinner label={t("common.loading")} />;

  const hasContent = history.sessions.length > 0 || history.deletions.length > 0;

  return (
    <div className="page">
      <div className="page-title">{t("history.title")}</div>
      <div className="page-subtitle">{t("history.subtitle")}</div>

      {!hasContent ? (
        <div className="empty">{t("history.empty")}</div>
      ) : (
        <>
          {history.sessions.length > 0 && (
            <div className="panel">
              <div className="panel-title">{t("history.sessions")}</div>
              {history.sessions.map((s, i) => (
                <div key={i} className="row">
                  <div className="row-main">
                    <div className="row-name">
                      {s.command}
                      <span className="tag blue">{t("history.items", { count: s.items })}</span>
                      {s.failed > 0 && <span className="tag red">{t("history.failed", { count: s.failed })}</span>}
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
              <div className="panel-title">{t("history.deletions")}</div>
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
          {t("common.refresh")}
        </button>
      </div>
      <div style={{ color: "var(--text-dim)", fontSize: 11, marginTop: 8 }}>
        {t("history.logAt", { path: history.operations_log })}
      </div>
    </div>
  );
}

