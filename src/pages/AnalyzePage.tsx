import { useCallback, useState } from "react";
import { scanDirectory } from "../api";
import { humanBytes } from "../api";
import type { ScanResult } from "../types";

export default function AnalyzePage() {
  const [path, setPath] = useState("");
  const [result, setResult] = useState<ScanResult | null>(null);
  const [history, setHistory] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);

  const analyze = useCallback(async (target: string, pushHistory = true) => {
    setScanning(true);
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
      <div className="page-title">磁盘分析</div>
      <div className="page-subtitle">
        输入路径查看逐层占用。构建产物（node_modules、target、DerivedData 等）会被标出，
        删掉重编译就能恢复。
      </div>

      <form
        className="toolbar"
        onSubmit={(e) => {
          e.preventDefault();
          if (path.trim()) analyze(path.trim());
        }}
      >
        <input
          className="input"
          placeholder="/Users/you/Projects"
          value={path}
          onChange={(e) => setPath(e.target.value)}
        />
        <button className="button" type="submit" disabled={scanning}>
          分析
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
      {scanning && <div className="loading pulse">正在扫描…</div>}

      {result && !scanning && (
        <>
          <div className="panel">
            <div className="panel-title" style={{ display: "flex", justifyContent: "space-between" }}>
              <span>{result.path}</span>
              <span>
                {humanBytes(result.total_size)} · {result.total_files} 项
                {result.scan_status === "partial" && " · 部分扫描"}
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
                    {e.is_build_artifact && <span className="tag green">构建产物</span>}
                    {e.protection_reason && (
                      <span className="tag red" title={e.path}>
                        {e.protection_reason}
                      </span>
                    )}
                    {e.age_days !== null && e.age_days >= 90 && (
                      <span className="tag yellow">90 天未动</span>
                    )}
                  </div>
                </div>
                <div className="row-size">{humanBytes(e.size)}</div>
              </div>
            ))}
          </div>

          <div style={{ display: "flex", gap: 8 }}>
            <button className="button secondary" onClick={goUp}>
              返回上级
            </button>
          </div>

          {result.large_files.length > 0 && (
            <div className="panel" style={{ marginTop: 16 }}>
              <div className="panel-title">大文件（超过 500MB，已排除受保护项）</div>
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
