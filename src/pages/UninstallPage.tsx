import { useCallback, useEffect, useState } from "react";
import { appData, runUninstall } from "../api";
import { humanBytes } from "../api";
import { useT } from "../i18n";
import { Spinner, InlineSpinner, ScanProgress, useRotatingDetail } from "../components/Loading";
import { RefreshIcon } from "../components/Icons";
import { appsCacheAge, invalidateAppsCache, loadApps, peekAppsCache } from "../appsCache";
import type { App, AppDataPath, CommandOutput } from "../types";

/** 把缓存年龄格式化成「N 分钟前」这类短文案。 */
function formatAge(ms: number): string {
  const minutes = Math.floor(ms / 60000);
  if (minutes < 1) return "<1m";
  if (minutes < 60) return `${minutes}m`;
  return `${Math.floor(minutes / 60)}h`;
}

export default function UninstallPage() {
  // 有缓存时直接用它渲染，避免每次进入页面都先闪一次扫描态。
  const [apps, setApps] = useState<App[] | null>(() => peekAppsCache());
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<App | null>(null);
  const [dataPaths, setDataPaths] = useState<AppDataPath[]>([]);
  const [loadingData, setLoadingData] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<CommandOutput | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [scope, setScope] = useState<"user" | "system">("user");
  const [cacheAge, setCacheAge] = useState<number | null>(() => appsCacheAge());
  const t = useT();

  // 扫描时轮播应用目录，让等待有推进感。
  const rotatingApp = useRotatingDetail(
    scanning ? ["/Applications", "/System/Applications", "/Applications/Utilities", "读取 bundle 信息", "统计应用体积"] : [],
    700,
  );

  /**
   * 取应用列表。
   *
   * `force` 为 true 时忽略缓存（点刷新按钮、卸载后失效），否则优先用缓存 ——
   * 侧栏切走会卸载本组件，缓存放在模块级才能在切回来时命中。
   */
  const load = useCallback(async (force = false) => {
    // 命中缓存时同步一下年龄显示。
    const cached = force ? null : peekAppsCache();
    if (cached) {
      setApps(cached);
      setCacheAge(appsCacheAge());
      setError(null);
      return;
    }

    setScanning(true);
    try {
      setApps(await loadApps(force));
      setCacheAge(appsCacheAge());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }, []);

  // 进入页面：命中缓存就立刻显示，不重复扫描。
  useEffect(() => {
    load();
  }, [load]);

  // 首次进入时的扫描态：标题下方显示进度，而不是空白页。
  if (!apps && !error) {
    return (
      <div className="page">
        <div className="page-title">{t("uninstall.title")}</div>
        <div className="page-subtitle">{t("uninstall.subtitleEntry")}</div>
        <ScanProgress
          phases={[
            {
              step: t("uninstall.scanning"),
              detail: rotatingApp ?? undefined,
            },
          ]}
          active={0}
        />
      </div>
    );
  }

  const selectApp = async (app: App) => {
    setSelected(app);
    setDataPaths([]);
    setLoadingData(true);
    try {
      setDataPaths(await appData(app.bundle_id));
    } catch {
      // 数据目录扫描失败不影响主流程。
    } finally {
      setLoadingData(false);
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
      // 卸载改变了已安装列表，缓存必须失效再重新扫描。
      invalidateAppsCache();
      await load(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  };

  if (error && !apps) return <div className="error-box">{error}</div>;
  if (!apps) return <Spinner label={t("clean.scan.step")} />;

  // 系统应用与用户应用分开看：系统应用一律不可卸载，混在一起会让列表噪音很大。
  const inScope = apps.filter((a) => (scope === "system" ? a.protected : !a.protected));

  const filtered = inScope.filter(
    (a) =>
      search === "" ||
      a.name.toLowerCase().includes(search.toLowerCase()) ||
      a.bundle_id.toLowerCase().includes(search.toLowerCase()),
  );

  // 两个标签的数量基于搜索后的结果，切换时能直接看到各有多少。
  const searched = apps.filter(
    (a) =>
      search === "" ||
      a.name.toLowerCase().includes(search.toLowerCase()) ||
      a.bundle_id.toLowerCase().includes(search.toLowerCase()),
  );
  const userCount = searched.filter((a) => !a.protected).length;
  const systemCount = searched.filter((a) => a.protected).length;

  return (
    <div className={`page ${selected ? "has-action-bar" : ""}`}>
      <div className="title-row">
        <div className="page-title">{t("uninstall.title")}</div>
        <button
          className="icon-button"
          onClick={() => load(true)}
          disabled={scanning}
          title={t("clean.rescan")}
          aria-label={t("clean.rescan")}
        >
          <RefreshIcon spinning={scanning} />
        </button>
      </div>
      <div className="page-subtitle">{t("uninstall.subtitle")}</div>

      {error && <div className="error-box">{error}</div>}

      <div className="toolbar">
        <div className="scope-tabs">
          <button
            className={`scope-tab ${scope === "user" ? "active" : ""}`}
            onClick={() => {
              setScope("user");
              setSelected(null);
              setConfirming(false);
            }}
          >
            {t("uninstall.tabUser")}
            <span className="scope-count">{userCount}</span>
          </button>
          <button
            className={`scope-tab ${scope === "system" ? "active" : ""}`}
            onClick={() => {
              setScope("system");
              setSelected(null);
              setConfirming(false);
            }}
          >
            {t("uninstall.tabSystem")}
            <span className="scope-count">{systemCount}</span>
          </button>
        </div>
        <input
          className="input"
          placeholder={t("uninstall.searchPlaceholder")}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      <div className="panel">
        <div className="panel-title">{t("uninstall.installed", { count: filtered.length })}</div>
        {filtered.length === 0 && (
          <div className="empty">
            {scope === "system" ? t("uninstall.emptySystem") : t("uninstall.emptyUser")}
          </div>
        )}
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
            {app.icon_png_base64 ? (
              <img
                className="app-icon"
                src={`data:image/png;base64,${app.icon_png_base64}`}
                alt=""
                aria-hidden="true"
              />
            ) : (
              // 取不到图标时用名字首字母占位，避免行高跳动。
              <span className="app-icon app-icon-placeholder" aria-hidden="true">
                {app.name.slice(0, 1).toUpperCase()}
              </span>
            )}
            <div className="row-main">
              <div className="row-name">
                {app.name}
                {app.version && <span className="row-path" style={{ display: "inline", marginLeft: 8 }}>{app.version}</span>}
                {app.source === "Homebrew" && <span className="tag blue">Homebrew</span>}
                {app.protected && <span className="tag red">{t("uninstall.protected")}</span>}
              </div>
              <div className="row-path">
                {app.bundle_id || app.path}
                {app.last_used_days_ago !== null && (
                  <span style={{ marginLeft: 8, color: app.last_used_days_ago > 90 ? "var(--yellow)" : "var(--text-dim)" }}>
                    {app.last_used_days_ago === 0
                      ? t("uninstall.usedToday")
                      : app.last_used_days_ago < 30
                        ? t("uninstall.daysAgo", { days: app.last_used_days_ago })
                        : t("uninstall.monthsAgo", { months: Math.round(app.last_used_days_ago / 30) })}
                  </span>
                )}
              </div>
            </div>
            <div className="row-size">{app.size_display}</div>
          </div>        ))}
      </div>

      {selected && (
        <div className="panel">
          <div className="panel-title">
            {t("uninstall.appData", { name: selected.name })}
            {selected.protected && (
              <span className="tag red" style={{ marginLeft: 8 }}>
                {selected.protection_reason}
              </span>
            )}
          </div>

          {loadingData ? (
            // 选中应用后展示正在查找哪个应用的数据，而不是无反馈。
            <div className="loading-block" style={{ padding: "20px 0" }}>
              <Spinner label={t("uninstall.selecting", { name: selected.name })} />
            </div>
          ) : dataPaths.length === 0 ? (
            <div className="empty">{t("uninstall.noData")}</div>
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
            <span style={{ color: "var(--text-dim)", alignSelf: "center", fontSize: 11 }}>
              {selected.protected
                ? t("uninstall.protected")
                : t("uninstall.actionBarHint")}
            </span>
          </div>

          {running && (
            // 卸载执行中：明确显示正在处理的应用名与其主程序路径。
            <div className="loading-block" style={{ padding: "16px 0" }}>
              <span className="spinner inline" aria-hidden="true" />
              <div className="loading-body">
                <span className="loading-label" style={{ fontSize: 12 }}>
                  {t("uninstall.running", { name: selected.name })}
                </span>
                <span className="loading-detail">{selected.path}</span>
              </div>
            </div>
          )}
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

      <div style={{ color: "var(--text-dim)", fontSize: 11, textAlign: "center", marginTop: 8 }}>
        {t("common.appCount", {
          count: apps.length,
          size: humanBytes(apps.reduce((s, a) => s + a.size, 0)),
        })}
        {cacheAge !== null && ` · ${t("uninstall.cachedAgo", { time: formatAge(cacheAge) })}`}
      </div>

      {selected && (
        // 悬浮在窗口底部：选中应用后立刻可见，不用滚到列表末尾。
        <div className="action-bar">
          <div className="action-bar-inner">
            {selected.icon_png_base64 ? (
              <img
                className="app-icon action-bar-icon"
                src={`data:image/png;base64,${selected.icon_png_base64}`}
                alt=""
                aria-hidden="true"
              />
            ) : (
              <span className="app-icon app-icon-placeholder action-bar-icon" aria-hidden="true">
                {selected.name.slice(0, 1).toUpperCase()}
              </span>
            )}
            <div className="action-bar-info">
              <span className="action-bar-name">{selected.name}</span>
              <span className="action-bar-detail">
                {running
                  ? t("uninstall.running", { name: selected.name })
                  : selected.protected
                    ? selected.protection_reason
                    : `${selected.size_display} · ${selected.path}`}
              </span>
            </div>

            {!confirming ? (
              <button
                className="button danger"
                disabled={selected.protected || loadingData || running}
                onClick={() => setConfirming(true)}
              >
                {t("uninstall.remove", { name: selected.name })}
              </button>
            ) : (
              <>
                <button
                  className="button secondary"
                  onClick={() => setConfirming(false)}
                  disabled={running}
                >
                  {t("common.cancel")}
                </button>
                <button className="button danger" onClick={uninstall} disabled={running}>
                  {running && <InlineSpinner />}
                  {running ? t("uninstall.running", { name: selected.name }) : t("uninstall.confirmTip")}
                </button>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

