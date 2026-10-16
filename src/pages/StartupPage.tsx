import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { openLoginItemsSettings, setStartupItem, startupItems } from "../api";
import { useT } from "../i18n";
import { Spinner, InlineSpinner, useRotatingDetail } from "../components/Loading";
import { RefreshIcon } from "../components/Icons";
import type { StartupItem } from "../types";

type StartupFilterScope = "all" | "login_item" | "services" | "user_agent" | "global_agent" | "daemon";

export default function StartupPage() {
  const [items, setItems] = useState<StartupItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [search, setSearch] = useState("");
  const [scope, setScope] = useState<StartupFilterScope>("all");
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

  // 各分类统计
  const counts = useMemo(() => {
    const all = items ?? [];
    const loginItems = all.filter((i) => i.kind === "login_item");
    const userAgents = all.filter((i) => i.kind === "user_agent");
    const globalAgents = all.filter((i) => i.kind === "global_agent");
    const daemons = all.filter((i) => i.kind === "daemon");
    const services = all.filter((i) => i.kind !== "login_item");

    return {
      all: all.length,
      login_item: loginItems.length,
      services: services.length,
      user_agent: userAgents.length,
      global_agent: globalAgents.length,
      daemon: daemons.length,
    };
  }, [items]);

  const query = search.trim().toLowerCase();
  const matched = useMemo(() => {
    return (items ?? []).filter((i) => {
      // 搜索过滤
      const matchesSearch =
        query === "" ||
        i.name.toLowerCase().includes(query) ||
        i.id.toLowerCase().includes(query) ||
        (i.developer ?? "").toLowerCase().includes(query) ||
        (i.sub_text ?? "").toLowerCase().includes(query);

      if (!matchesSearch) return false;

      // Tab 分类过滤
      if (scope === "all") return true;
      if (scope === "login_item") return i.kind === "login_item";
      if (scope === "services") return i.kind !== "login_item";
      return i.kind === scope;
    });
  }, [items, query, scope]);

  // 根据当前 scope 构建面板分组
  const panels = useMemo(() => {
    if (scope === "all") {
      const loginItems = matched.filter((i) => i.kind === "login_item");
      const serviceItems = matched.filter((i) => i.kind !== "login_item");
      const list = [];
      if (loginItems.length > 0) {
        list.push({
          key: "login_item",
          title: t("startup.filter.background"),
          count: loginItems.length,
          items: loginItems,
        });
      }
      if (serviceItems.length > 0) {
        list.push({
          key: "services",
          title: t("startup.filter.service"),
          count: serviceItems.length,
          items: serviceItems,
        });
      }
      return list;
    }

    if (scope === "services") {
      const userAgents = matched.filter((i) => i.kind === "user_agent");
      const globalAgents = matched.filter((i) => i.kind === "global_agent");
      const daemons = matched.filter((i) => i.kind === "daemon");
      const list = [];
      if (userAgents.length > 0) {
        list.push({
          key: "user_agent",
          title: t("startup.filter.user_agent"),
          count: userAgents.length,
          items: userAgents,
        });
      }
      if (globalAgents.length > 0) {
        list.push({
          key: "global_agent",
          title: t("startup.filter.global_agent"),
          count: globalAgents.length,
          items: globalAgents,
        });
      }
      if (daemons.length > 0) {
        list.push({
          key: "daemon",
          title: t("startup.filter.daemon"),
          count: daemons.length,
          items: daemons,
        });
      }
      return list;
    }

    const titleMap: Record<StartupFilterScope, string> = {
      all: t("startup.filter.all"),
      login_item: t("startup.filter.background"),
      services: t("startup.filter.service"),
      user_agent: t("startup.filter.user_agent"),
      global_agent: t("startup.filter.global_agent"),
      daemon: t("startup.filter.daemon"),
    };

    return [
      {
        key: scope,
        title: titleMap[scope],
        count: matched.length,
        items: matched,
      },
    ];
  }, [matched, scope, t]);

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
            <div className="scope-tabs">
              <button
                className={`scope-tab ${scope === "all" ? "active" : ""}`}
                onClick={() => setScope("all")}
              >
                {t("startup.filter.all")}
                <span className="scope-count">{counts.all}</span>
              </button>
              <button
                className={`scope-tab ${scope === "login_item" ? "active" : ""}`}
                onClick={() => setScope("login_item")}
              >
                {t("startup.filter.background")}
                <span className="scope-count">{counts.login_item}</span>
              </button>
              <button
                className={`scope-tab ${scope === "services" ? "active" : ""}`}
                onClick={() => setScope("services")}
              >
                {t("startup.filter.service")}
                <span className="scope-count">{counts.services}</span>
              </button>
              <button
                className={`scope-tab ${scope === "user_agent" ? "active" : ""}`}
                onClick={() => setScope("user_agent")}
              >
                {t("startup.filter.user_agent")}
                <span className="scope-count">{counts.user_agent}</span>
              </button>
              <button
                className={`scope-tab ${scope === "global_agent" ? "active" : ""}`}
                onClick={() => setScope("global_agent")}
              >
                {t("startup.filter.global_agent")}
                <span className="scope-count">{counts.global_agent}</span>
              </button>
              <button
                className={`scope-tab ${scope === "daemon" ? "active" : ""}`}
                onClick={() => setScope("daemon")}
              >
                {t("startup.filter.daemon")}
                <span className="scope-count">{counts.daemon}</span>
              </button>
            </div>
            <input
              className="input"
              placeholder={t("startup.searchPlaceholder")}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>

          {panels.length === 0 || matched.length === 0 ? (
            <div className="empty">
              {query === "" ? t("startup.emptyAll") : t("startup.empty")}
            </div>
          ) : (
            panels.map((panel) => (
              <div className="panel" key={panel.key}>
                <div className="panel-title">
                  {panel.title}
                  <span className="scope-count" style={{ marginLeft: 8 }}>
                    {panel.count}
                  </span>
                </div>
                {panel.items.map((item) => (
                  <div key={item.id} className="row">
                    {item.icon_png_base64 ? (
                      <img
                        className="app-icon"
                        src={`data:image/png;base64,${item.icon_png_base64}`}
                        alt=""
                        aria-hidden="true"
                      />
                    ) : (
                      <span className="app-icon app-icon-placeholder" aria-hidden="true">
                        {item.name.slice(0, 1).toUpperCase()}
                      </span>
                    )}
                    <div className="row-main">
                      <div className="row-name" style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <span>{item.name}</span>
                        {item.sub_text && (
                          <span className="tag" style={{ fontSize: 11, padding: "2px 6px" }}>
                            {item.sub_text}
                          </span>
                        )}
                        {!item.toggleable && (
                          <span className="tag blue" title={item.reason ?? ""}>
                            {t("startup.managed")}
                          </span>
                        )}
                      </div>
                      <div className="row-path">
                        {item.developer ? (
                          <span>
                            <strong>{item.developer}</strong>
                            {item.id !== item.name && (
                              <span style={{ opacity: 0.7, marginLeft: 8 }}>· {item.id}</span>
                            )}
                          </span>
                        ) : (
                          item.id
                        )}
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
