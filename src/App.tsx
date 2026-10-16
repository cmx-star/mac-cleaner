import { useEffect, useState } from "react";
import { cliStatus } from "./api";
import { useT, toggleLocale, getLocale } from "./i18n";
import type { CliLocation } from "./types";
import StatusPage from "./pages/StatusPage";
import CleanPage from "./pages/CleanPage";
import OptimizePage from "./pages/OptimizePage";
import UninstallPage from "./pages/UninstallPage";
import StartupPage from "./pages/StartupPage";
import AnalyzePage from "./pages/AnalyzePage";
import HistoryPage from "./pages/HistoryPage";

type Tab = "status" | "clean" | "optimize" | "uninstall" | "startup" | "analyze" | "history";

const TABS: { id: Tab; icon: string; key: "nav.status" | "nav.clean" | "nav.optimize" | "nav.uninstall" | "nav.startup" | "nav.analyze" | "nav.history" }[] = [
  { id: "status", icon: "◉", key: "nav.status" },
  { id: "clean", icon: "✦", key: "nav.clean" },
  { id: "optimize", icon: "⚡", key: "nav.optimize" },
  { id: "uninstall", icon: "◎", key: "nav.uninstall" },
  { id: "startup", icon: "⏻", key: "nav.startup" },
  { id: "analyze", icon: "▤", key: "nav.analyze" },
  { id: "history", icon: "⏱", key: "nav.history" },
];

export default function App() {
  const [tab, setTab] = useState<Tab>("status");
  const [cli, setCli] = useState<CliLocation | null>(null);
  const t = useT();

  useEffect(() => {
    cliStatus().then(setCli).catch(() => setCli(null));
  }, []);

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">T</span>
          <span className="brand-name">Talpa</span>
          <button className="lang-toggle" onClick={toggleLocale} title="Language">
            {getLocale() === "zh-CN" ? "EN" : "中"}
          </button>
        </div>
        <nav className="nav">
          {TABS.map((tab_def) => (
            <button
              key={tab_def.id}
              className={`nav-item ${tab === tab_def.id ? "active" : ""}`}
              onClick={() => setTab(tab_def.id)}
            >
              <span className="nav-icon">{tab_def.icon}</span>
              {t(tab_def.key)}
            </button>
          ))}
        </nav>
        <div className="sidebar-footer">
          <span
            className="cli-badge ok"
            title={cli?.path ? `${cli.path} · 原生引擎` : "原生安全引擎已就绪"}
            style={{ display: "inline-flex", alignItems: "center" }}
          >
            <span
              style={{
                width: 6,
                height: 6,
                borderRadius: "50%",
                background: "var(--green)",
                display: "inline-block",
                marginRight: 6,
                boxShadow: "0 0 6px rgba(77, 182, 172, 0.6)",
              }}
            />
            {cli?.version || "原生引擎"}
          </span>
        </div>
      </aside>
      <main className="content">
        {tab === "status" && <StatusPage />}
        {tab === "clean" && <CleanPage />}
        {tab === "optimize" && <OptimizePage />}
        {tab === "uninstall" && <UninstallPage />}
        {tab === "startup" && <StartupPage />}
        {tab === "analyze" && <AnalyzePage />}
        {tab === "history" && <HistoryPage />}
      </main>
    </div>
  );
}
