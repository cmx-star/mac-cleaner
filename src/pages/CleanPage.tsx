import { useCallback, useState } from "react";
import { cleanupPreview, runClean } from "../api";
import { humanBytes } from "../api";
import { useT } from "../i18n";
import { ScanProgress, InlineSpinner, useRotatingDetail } from "../components/Loading";
import type { CleanupPreview, CommandOutput, Tier } from "../types";

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

export default function CleanPage() {
  const [preview, setPreview] = useState<CleanupPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [running, setRunning] = useState(false);
  const [runningName, setRunningName] = useState<string | null>(null);
  const [result, setResult] = useState<CommandOutput | null>(null);
  const [showCliHint, setShowCliHint] = useState(false);
  const t = useT();

  // 扫描中轮播展示常见缓存目录，让等待有「在推进」的感觉。
  const watchDirs = [
    "~/Library/Caches",
    "~/.cache",
    "~/Library/Logs",
    "~/Library/Application Support",
    "node_modules",
    "DerivedData",
  ];
  const rotatingDir = useRotatingDetail(scanning ? watchDirs : []);

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

  if (!preview && !scanning && !error) {
    return (
      <div className="page">
        <div className="page-title">{t("clean.title")}</div>
        <div className="page-subtitle">{t("clean.subtitleEntry")}</div>
        <button className="button" onClick={load}>
          {t("clean.startScan")}
        </button>
      </div>
    );
  }

  const execute = async () => {
    // 先做 dry-run 让 Mole 的安全层自己确认一遍。
    setRunning(true);
    setRunningName(null);
    setResult(null);
    try {
      setRunningName(t("clean.dryrun"));
      const dry = await runClean(true);
      if (!dry.success) {
        setError(dry.stderr || "dry-run 未通过，已停止执行");
        return;
      }
      setRunningName(t("clean.deleting", { name: "…" }));
      const real = await runClean(false);
      setResult(real);
      await load();
    } catch (e) {
      const message = String(e);
      setError(message);
      if (message.includes("没有找到") || message.includes("install")) {
        setShowCliHint(true);
      }
    } finally {
      setRunning(false);
      setRunningName(null);
    }
  };

  if (scanning && !preview) {
    return (
      <div className="page">
        <div className="page-title">{t("clean.title")}</div>
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

  return (
    <div className="page">
      <div className="page-title">{t("clean.title")}</div>
      <div className="page-subtitle">{t("clean.subtitle")}</div>

      {error && <div className="error-box">{error}</div>}
      {showCliHint && <div className="hint-box">{t("clean.cliHint")}</div>}

      {preview && (
        <>
          <div className="panel">
            <div className="panel-title">{t("clean.overview")}</div>
            <div style={{ display: "flex", gap: 32 }}>
              <div>
                <div className="stat-label">{t("clean.regenerable")}</div>
                <div className="big-number">{humanBytes(preview.total_regenerable)}</div>
              </div>
              <div>
                <div className="stat-label">{t("clean.whitelist")}</div>
                <div className="big-number">{preview.whitelist_count}</div>
              </div>
            </div>
          </div>

          <div className="panel">
            <div className="panel-title">{t("clean.candidates")}</div>
            {preview.items.length === 0 ? (
              <div className="empty">{t("clean.empty")}</div>
            ) : (
              preview.items.slice(0, 50).map((item) => (
                <div key={item.path} className="row">
                  <div className="row-main">
                    <div className="row-name">
                      {item.name}
                      <span className={`tag ${TIER_CLASSES[item.tier]}`}>
                        {t(TIER_KEYS[item.tier])}
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
              {scanning && <InlineSpinner />}
              {scanning ? t("clean.scanningShort") : t("clean.rescan")}
            </button>
            <button
              className="button danger"
              onClick={execute}
              disabled={running || preview.items.filter((i) => i.tier === "regenerable").length === 0}
            >
              {running && <InlineSpinner />}
              {running ? t("clean.executing") : t("clean.execute")}
            </button>
          </div>

          {running && runningName && (
            <div className="loading-block" style={{ padding: "16px 0" }}>
              <span className="spinner inline" aria-hidden="true" />
              <span className="loading-label" style={{ fontSize: 12 }}>{runningName}</span>
            </div>
          )}

          {result && (
            <div className="panel">
              <div className="panel-title">
                {t("common.exitCode", { code: result.exit_code ?? "—" })}
              </div>
              <div className="output-box">{result.stdout || result.stderr || t("common.noOutput")}</div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

