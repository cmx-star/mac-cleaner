import { useCallback, useEffect, useState } from "react";
import { cleanupPreview, runClean } from "../api";
import { humanBytes } from "../api";
import type { CleanupPreview, CommandOutput, Tier } from "../types";

const TIER_LABELS: Record<Tier, string> = {
  regenerable: "可再生",
  expensive: "重建代价高",
  irreplaceable: "不可替代",
};

const TIER_CLASSES: Record<Tier, string> = {
  regenerable: "green",
  expensive: "yellow",
  irreplaceable: "red",
};

export default function CleanPage() {
  const [preview, setPreview] = useState<CleanupPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<CommandOutput | null>(null);
  const [showCliHint, setShowCliHint] = useState(false);

  const load = useCallback(async () => {
    setScanning(true);
    try {
      setPreview(await cleanupPreview());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const execute = async () => {
    // 先做 dry-run 让 Mole 的安全层自己确认一遍。
    setRunning(true);
    setResult(null);
    try {
      const dry = await runClean(true);
      if (!dry.success) {
        setError(dry.stderr || "dry-run 未通过，已停止执行");
        return;
      }
      const real = await runClean(false);
      setResult(real);
      await load();
    } catch (e) {
      const message = String(e);
      setError(message);
      // CLI 未安装时提示用户。
      if (message.includes("没有找到") || message.includes("install")) {
        setShowCliHint(true);
      }
    } finally {
      setRunning(false);
    }
  };

  if (scanning && !preview) return <div className="loading pulse">正在扫描可清理项…</div>;

  return (
    <div className="page">
      <div className="page-title">清理</div>
      <div className="page-subtitle">
        先看一眼再删。扫描在本机完成，删除由 Mole CLI 的安全层执行并进废纸篓。
      </div>

      {error && <div className="error-box">{error}</div>}
      {showCliHint && (
        <div className="hint-box">
          执行清理需要 Mole CLI。安装：brew install mole
        </div>
      )}

      {preview && (
        <>
          <div className="panel">
            <div className="panel-title">概览</div>
            <div style={{ display: "flex", gap: 32 }}>
              <div>
                <div className="stat-label">可再生项最多可释放</div>
                <div className="big-number">{humanBytes(preview.total_regenerable)}</div>
              </div>
              <div>
                <div className="stat-label">保护名单条目</div>
                <div className="big-number">{preview.whitelist_count}</div>
              </div>
            </div>
          </div>

          <div className="panel">
            <div className="panel-title">清理候选（按体积排序）</div>
            {preview.items.length === 0 ? (
              <div className="empty">没有找到可清理项，磁盘很干净。</div>
            ) : (
              preview.items.slice(0, 50).map((item) => (
                <div key={item.path} className="row">
                  <div className="row-main">
                    <div className="row-name">
                      {item.name}
                      <span className={`tag ${TIER_CLASSES[item.tier]}`}>
                        {TIER_LABELS[item.tier]}
                      </span>
                      {item.protection_reason && (
                        <span className="tag blue" title={item.path}>
                          {item.protection_reason}
                        </span>
                      )}
                    </div>
                    <div className="row-path">{item.path}</div>
                  </div>
                  <div className="row-size">{humanBytes(item.size)}</div>
                </div>
              ))
            )}
          </div>

          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <button className="button secondary" onClick={load} disabled={scanning}>
              重新扫描
            </button>
            <button
              className="button danger"
              onClick={execute}
              disabled={running || preview.items.filter((i) => i.tier === "regenerable").length === 0}
            >
              {running ? "执行中…" : "执行清理"}
            </button>
          </div>

          {result && (
            <div className="panel">
              <div className="panel-title">执行结果（退出码 {result.exit_code ?? "无"}）</div>
              <div className="output-box">{result.stdout || result.stderr || "无输出"}</div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
