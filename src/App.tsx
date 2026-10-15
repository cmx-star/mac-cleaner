import { useEffect, useState } from "react";
import { cliStatus } from "./api";
import { useT, toggleLocale, getLocale } from "./i18n";
import type { CliLocation } from "./types";
import StatusPage from "./pages/StatusPage";
import CleanPage from "./pages/CleanPage";
import UninstallPage from "./pages/UninstallPage";
import StartupPage from "./pages/StartupPage";
import AnalyzePage from "./pages/AnalyzePage";
import HistoryPage from "./pages/HistoryPage";

type Tab = "status" | "clean" | "uninstall" | "startup" | "analyze" | "history";

const TABS: { id: Tab; icon: string; key: "nav.status" | "nav.clean" | "nav.uninstall" | "nav.startup" | "nav.analyze" | "nav.history" }[] = [
  { id: "status", icon: "◉", key: "nav.status" },
  { id: "clean", icon: "✦", key: "nav.clean" },
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
          {cli?.found ? (
            <span className="cli-badge ok" title={cli.path ?? ""}>
              {cli.version ?? t("sidebar.cliMissing")}
            </span>
          ) : (
            <span className="cli-badge missing" title={cli?.install_hint ?? ""}>
              {t("sidebar.cliMissing")}
            </span>
          )}
        </div>
      </aside>
      <main className="content">
        {tab === "status" && <StatusPage />}
        {tab === "clean" && <CleanPage />}
        {tab === "uninstall" && <UninstallPage />}
        {tab === "startup" && <StartupPage />}
        {tab === "analyze" && <AnalyzePage />}
        {tab === "history" && <HistoryPage />}
      </main>
    </div>
  );
}
