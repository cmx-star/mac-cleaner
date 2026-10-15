import { useEffect, useState } from "react";
import { cliStatus } from "./api";
import type { CliLocation } from "./types";
import StatusPage from "./pages/StatusPage";
import CleanPage from "./pages/CleanPage";
import UninstallPage from "./pages/UninstallPage";
import AnalyzePage from "./pages/AnalyzePage";
import HistoryPage from "./pages/HistoryPage";

type Tab = "status" | "clean" | "uninstall" | "analyze" | "history";

const TABS: { id: Tab; label: string; icon: string }[] = [
  { id: "status", label: "状态", icon: "◉" },
  { id: "clean", label: "清理", icon: "✦" },
  { id: "uninstall", label: "卸载", icon: "◎" },
  { id: "analyze", label: "分析", icon: "▤" },
  { id: "history", label: "历史", icon: "⏱" },
];

export default function App() {
  const [tab, setTab] = useState<Tab>("status");
  const [cli, setCli] = useState<CliLocation | null>(null);

  useEffect(() => {
    cliStatus().then(setCli).catch(() => setCli(null));
  }, []);

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">M</span>
          <span className="brand-name">Mole</span>
        </div>
        <nav className="nav">
          {TABS.map((t) => (
            <button
              key={t.id}
              className={`nav-item ${tab === t.id ? "active" : ""}`}
              onClick={() => setTab(t.id)}
            >
              <span className="nav-icon">{t.icon}</span>
              {t.label}
            </button>
          ))}
        </nav>
        <div className="sidebar-footer">
          {cli?.found ? (
            <span className="cli-badge ok" title={cli.path ?? ""}>
              {cli.version ?? "CLI 已就绪"}
            </span>
          ) : (
            <span className="cli-badge missing" title={cli?.install_hint ?? ""}>
              CLI 未安装
            </span>
          )}
        </div>
      </aside>
      <main className="content">
        {tab === "status" && <StatusPage />}
        {tab === "clean" && <CleanPage />}
        {tab === "uninstall" && <UninstallPage />}
        {tab === "analyze" && <AnalyzePage />}
        {tab === "history" && <HistoryPage />}
      </main>
    </div>
  );
}
