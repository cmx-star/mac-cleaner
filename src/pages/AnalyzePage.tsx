import { useCallback, useEffect, useState } from "react";
import { homeDir, scanDirectory } from "../api";
import { humanBytes } from "../api";
import { useT } from "../i18n";
import { ScanProgress, InlineSpinner, useRotatingDetail } from "../components/Loading";
import type { ScanResult } from "../types";

export default function AnalyzePage() {
  const [path, setPath] = useState("");
  const [result, setResult] = useState<ScanResult | null>(null);
  const [history, setHistory] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [scanningPath, setScanningPath] = useState("");
  const t = useT();

  // 进入页面填入主目录作为默认路径，但不自动扫描，由用户点「分析」触发。
  useEffect(() => {
    homeDir()
      .then((dir) => {
        if (dir) setPath((current) => (current === "" ? dir : current));
      })
      .catch(() => {
        // 取不到主目录时保留空输入，用户自己填。
      });
  }, []);

  // 扫描中轮播的目录名细节，让深层扫描有推进感。
  const rotating = useRotatingDetail(
    scanning ? ["扫描目录", "统计体积", "识别构建产物", "检查大文件"] : [],
    900,
  );

  const analyze = useCallback(async (target: string, pushHistory = true) => {
    setScanning(true);
    setScanningPath(target);
    setError(null);
    try {
      const r = await scanDirectory(target);
      setResult(r);
      if (pushHistory) {
        setHistory((h) => [...h.filter((p) => p !== target), target].slice(-8));
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }, []);

  const goUp = () => {
    if (!result) return;
    const parent = result.path.replace(/\/[^/]+$/, "") || "/";
    analyze(parent);
  };

  const navigate = (target: string, isDir: boolean) => {
    if (isDir) analyze(target);
  };

  return (
    <div className="page">
      <div className="page-title">{t("analyze.title")}</div>
      <div className="page-subtitle">{t("analyze.subtitle")}</div>

      <form
        className="toolbar"
        onSubmit={(e) => {
          e.preventDefault();
          const target = path.trim();
          if (!target) {
            // 空输入不静默无反应：明确提示而不是让用户以为按钮失灵。
            setError(t("analyze.emptyPath"));
            return;
          }
          analyze(target);
        }}
      >
        <input
          className="input"
          placeholder={t("analyze.placeholder")}
          value={path}
          onChange={(e) => setPath(e.target.value)}
          disabled={scanning}
        />
        <button className="button" type="submit" disabled={scanning}>
          {scanning && <InlineSpinner />}
          {scanning ? t("analyze.scanningShort") : t("analyze.go")}
        </button>
      </form>

      {history.length > 0 && (
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 16 }}>
          {history.map((h) => (
            <button
              key={h}
              className="button secondary"
              style={{ padding: "4px 10px", fontSize: 11 }}
              onClick={() => analyze(h, false)}
            >
              {h.replace(/^\/Users\/[^/]+/, "~")}
            </button>
          ))}
        </div>
      )}

      {error && <div className="error-box">{error}</div>}
      {scanning && (
        <ScanProgress
          phases={[
            {
              step: t("analyze.scanning", { path: scanningPath }),
              detail: rotating ?? undefined,
            },
          ]}
          active={0}
        />
      )}

      {result && !scanning && (
        <>
          <div className="panel">
            <div className="panel-title" style={{ display: "flex", justifyContent: "space-between" }}>
              <span>{result.path}</span>
              <span>
                {humanBytes(result.total_size)} · {result.total_files} 项
                {result.scan_status === "partial" && ` · ${t("analyze.partial")}`}
              </span>
            </div>
            {result.entries.map((e) => (
              <div
                key={e.path}
                className="row"
                style={{ cursor: e.is_dir ? "pointer" : "default" }}
                onClick={() => navigate(e.path, e.is_dir)}
              >
                <div className="row-main">
                  <div className="row-name">
                    {e.is_dir ? "▸ " : "  "}
                    {e.name}
                    {e.is_build_artifact && <span className="tag green">{t("analyze.buildArtifact")}</span>}
                    {e.protection_reason && (
                      <span className="tag red" title={e.path}>
                        {e.protection_reason}
                      </span>
                    )}
                    {e.age_days !== null && e.age_days >= 90 && (
                      <span className="tag yellow">{t("analyze.stale", { days: e.age_days })}</span>
                    )}
                  </div>
                </div>
                <div className="row-size">{humanBytes(e.size)}</div>
              </div>
            ))}
          </div>

          <div style={{ display: "flex", gap: 8 }}>
            <button className="button secondary" onClick={goUp}>
              {t("analyze.up")}
            </button>
          </div>

          {result.large_files.length > 0 && (
            <div className="panel" style={{ marginTop: 16 }}>
              <div className="panel-title">{t("analyze.largeFiles")}</div>
              {result.large_files.map((f) => (
                <div key={f.path} className="row">
                  <div className="row-main">
                    <div className="row-name">{f.name}</div>
                    <div className="row-path">{f.path}</div>
                  </div>
                  <div className="row-size">{humanBytes(f.size)}</div>
                </div>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}

