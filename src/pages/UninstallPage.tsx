import { useCallback, useEffect, useState } from "react";
import { appData, listApps, runUninstall } from "../api";
import { humanBytes } from "../api";
import type { App, AppDataPath, CommandOutput } from "../types";

export default function UninstallPage() {
  const [apps, setApps] = useState<App[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<App | null>(null);
  const [dataPaths, setDataPaths] = useState<AppDataPath[]>([]);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<CommandOutput | null>(null);
  const [confirming, setConfirming] = useState(false);

  const load = useCallback(async () => {
    try {
      setApps(await listApps());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const selectApp = async (app: App) => {
    setSelected(app);
    setDataPaths([]);
    try {
      setDataPaths(await appData(app.bundle_id));
    } catch {
      // 数据目录扫描失败不影响主流程。
    }
  };

  const uninstall = async () => {
    if (!selected) return;
    setRunning(true);
    setResult(null);
    try {
      const out = await runUninstall(selected.name, false);
      setResult(out);
      setSelected(null);
      setConfirming(false);
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  };

  if (error && !apps) return <div className="error-box">{error}</div>;
  if (!apps) return <div className="loading pulse">正在扫描已安装应用…</div>;

  const filtered = apps.filter(
    (a) =>
      search === "" ||
      a.name.toLowerCase().includes(search.toLowerCase()) ||
      a.bundle_id.toLowerCase().includes(search.toLowerCase()),
  );

  return (
    <div className="page">
      <div className="page-title">卸载</div>
      <div className="page-subtitle">
        应用本体的体积常常不是大头，选中应用可以看到它散落在系统各处的数据。
        删除由 Mole CLI 执行，默认进废纸篓。
      </div>

      {error && <div className="error-box">{error}</div>}

      <div className="toolbar">
        <input
          className="input"
          placeholder="搜索应用名或 bundle id…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      <div className="panel">
        <div className="panel-title">已安装应用 · {filtered.length} 个</div>
        {filtered.map((app) => (
          <div
            key={app.path}
            className="row"
            style={{
              cursor: "pointer",
              background: selected?.path === app.path ? "var(--bg-hover)" : undefined,
              borderRadius: 6,
            }}
            onClick={() => selectApp(app)}
          >
            <div className="row-main">
              <div className="row-name">
                {app.name}
                {app.source === "Homebrew" && <span className="tag blue">Homebrew</span>}
                {app.protected && <span className="tag red">系统</span>}
              </div>
              <div className="row-path">{app.bundle_id || app.path}</div>
            </div>
            <div className="row-size">{app.size_display}</div>
          </div>
        ))}
      </div>

      {selected && (
        <div className="panel">
          <div className="panel-title">
            {selected.name} 的相关数据
            {selected.protected && (
              <span className="tag red" style={{ marginLeft: 8 }}>
                {selected.protection_reason}
              </span>
            )}
          </div>
          {dataPaths.length === 0 ? (
            <div className="empty">没有找到散落的应用数据。</div>
          ) : (
            dataPaths.map((p) => (
              <div key={p.path} className="row">
                <div className="row-main">
                  <div className="row-path" style={{ fontSize: 12 }}>{p.path}</div>
                  {p.protection_reason && (
                    <div className="row-path" style={{ color: "var(--yellow)" }}>
                      {p.protection_reason}
                    </div>
                  )}
                </div>
                <div className="row-size">{p.size_display}</div>
              </div>
            ))
          )}

          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end", marginTop: 12 }}>
            {!confirming ? (
              <button
                className="button danger"
                disabled={selected.protected}
                onClick={() => setConfirming(true)}
              >
                卸载 {selected.name}
              </button>
            ) : (
              <>
                <span style={{ color: "var(--yellow)", alignSelf: "center" }}>
                  确认卸载？文件将进废纸篓。
                </span>
                <button className="button secondary" onClick={() => setConfirming(false)}>
                  取消
                </button>
                <button className="button danger" onClick={uninstall} disabled={running}>
                  {running ? "执行中…" : "确认"}
                </button>
              </>
            )}
          </div>
        </div>
      )}

      {result && (
        <div className="panel">
          <div className="panel-title">执行结果（退出码 {result.exit_code ?? "无"}）</div>
          <div className="output-box">{result.stdout || result.stderr || "无输出"}</div>
        </div>
      )}

      <div style={{ color: "var(--text-dim)", fontSize: 11, textAlign: "center", marginTop: 8 }}>
        共 {apps.length} 个应用 · 合计 {humanBytes(apps.reduce((s, a) => s + a.size, 0))}
      </div>
    </div>
  );
}
