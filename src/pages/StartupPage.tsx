import { useCallback, useEffect, useRef, useState } from "react";
import { openLoginItemsSettings, setStartupItem, startupItems } from "../api";
import { useT } from "../i18n";
import { Spinner, InlineSpinner, useRotatingDetail } from "../components/Loading";
import { RefreshIcon } from "../components/Icons";
import type { StartupItem, StartupItemKind } from "../types";

/** 分组顺序与界面展示顺序一致。 */
const GROUP_ORDER: StartupItemKind[] = ["login_item", "background", "service"];

const GROUP_LABEL_KEYS: Record<
  StartupItemKind,
  "startup.group.login_item" | "startup.group.background" | "startup.group.service"
> = {
  login_item: "startup.group.login_item",
  background: "startup.group.background",
  service: "startup.group.service",
};

export default function StartupPage() {
  const [items, setItems] = useState<StartupItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [search, setSearch] = useState("");
  // 正在切换中的项 id：用于把对应开关置为忙碌态，避免重复点击。
  const [busyId, setBusyId] = useState<string | null>(null);
  const t = useT();

  const rotating = useRotatingDetail(
    scanning ? ["读取登录项", "读取后台项", "读取后台服务"] : [],
    700,
  );

  const load = useCallback(async () => {
    setScanning(true);
    try {
      setItems(await startupItems());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }, []);

  // 首次进入时拉取一次。采用零权限只读扫描，无管理员弹窗。
  const loadedOnce = useRef(false);
  useEffect(() => {
    if (loadedOnce.current) return;
    loadedOnce.current = true;
    load();
  }, [load]);

  const toggle = async (item: StartupItem) => {
    setBusyId(item.id);
    setError(null);
    try {
      await setStartupItem(item.id, !item.enabled);
      // 重新读取而不是就地翻转：系统可能拒绝这次改动，
      // 以实际状态为准才不会显示成「已关闭但实际还开着」。
      setItems(await startupItems());
    } catch (e) {
      setError(t("startup.toggleFailed", { message: String(e) }));
    } finally {
      setBusyId(null);
    }
  };

  const openSettings = async () => {
    try {
      await openLoginItemsSettings();
    } catch (e) {
      setError(String(e));
    }
  };

  const query = search.trim().toLowerCase();
  const matched = (items ?? []).filter(
    (i) =>
      query === "" ||
      i.name.toLowerCase().includes(query) ||
      i.id.toLowerCase().includes(query) ||
      (i.developer ?? "").toLowerCase().includes(query),
  );

  const grouped = GROUP_ORDER.map((kind) => ({
    kind,
    items: matched.filter((i) => i.kind === kind),
  })).filter((g) => g.items.length > 0);

  return (
    <div className="page">
      <div className="title-row">
        <div className="page-title">{t("startup.title")}</div>
        <button
          className="icon-button"
          onClick={load}
          disabled={scanning}
          title={t("clean.rescan")}
          aria-label={t("clean.rescan")}
        >
          <RefreshIcon spinning={scanning} />
        </button>
      </div>
      <div className="page-subtitle">{t("startup.subtitle")}</div>

      {error && <div className="error-box">{error}</div>}

      {!items && !error && (
        <Spinner label={rotating ? `${t("startup.scanning")} ${rotating}` : t("startup.scanning")} />
      )}

      {items && (
        <>
          <div className="toolbar">
            <input
              className="input"
              placeholder={t("startup.searchPlaceholder")}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>

          {grouped.length === 0 ? (
            <div className="empty">
              {query === "" ? t("startup.emptyAll") : t("startup.empty")}
            </div>
          ) : (
            grouped.map((group) => (
              <div className="panel" key={group.kind}>
                <div className="panel-title">
                  {t(GROUP_LABEL_KEYS[group.kind])}
                  <span className="scope-count" style={{ marginLeft: 8 }}>
                    {group.items.length}
                  </span>
                </div>
                {group.items.map((item) => (
                  <div key={item.id} className="row">
                    {item.icon_png_base64 ? (
                      <img
                        className="app-icon"
                        src={`data:image/png;base64,${item.icon_png_base64}`}
                        alt=""
                        aria-hidden="true"
                      />
                    ) : (
                      // launchd plist 类的项没有 bundle 图标，用首字母占位。
                      <span className="app-icon app-icon-placeholder" aria-hidden="true">
                        {item.name.slice(0, 1).toUpperCase()}
                      </span>
                    )}
                    <div className="row-main">
                      <div className="row-name">
                        {item.name}
                        {!item.toggleable && (
                          <span className="tag blue" title={item.reason ?? ""}>
                            {t("startup.managed")}
                          </span>
                        )}
                      </div>
                      <div className="row-path">
                        {item.developer ?? item.id}
                      </div>
                    </div>

                    {item.toggleable ? (
                      <label className="switch" title={item.enabled ? t("startup.enabled") : t("startup.disabled")}>
                        <input
                          type="checkbox"
                          className="switch-input"
                          checked={item.enabled}
                          disabled={busyId === item.id}
                          onChange={() => toggle(item)}
                        />
                        <span className="switch-track" aria-hidden="true">
                          <span className="switch-thumb" />
                        </span>
                        {busyId === item.id && <InlineSpinner />}
                      </label>
                    ) : (
                      // 系统托管的项改不了，给一个能直达系统设置的入口，
                      // 而不是放一个点了没反应的开关。
                      <button
                        className="button secondary small"
                        onClick={openSettings}
                        title={item.reason ?? ""}
                      >
                        {t("startup.openSettings")}
                      </button>
                    )}
                  </div>
                ))}
              </div>
            ))
          )}
        </>
      )}
    </div>
  );
}
