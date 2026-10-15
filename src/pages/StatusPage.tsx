import { useCallback, useEffect, useState } from "react";
import { systemStatus } from "../api";
import { humanBytes } from "../api";
import type { Snapshot } from "../types";

function Bar({ percent }: { percent: number }) {
  const cls = percent > 90 ? "danger" : percent > 75 ? "warn" : "";
  return (
    <div className="bar">
      <div className={`bar-fill ${cls}`} style={{ width: `${Math.min(percent, 100)}%` }} />
    </div>
  );
}

export default function StatusPage() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      setSnap(await systemStatus());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, 5000);
    return () => clearInterval(timer);
  }, [refresh]);

  if (error) return <div className="error-box">{error}</div>;
  if (!snap) return <div className="loading pulse">正在采集系统状态…</div>;

  const primaryDisk = snap.disks.find((d) => d.mount === "/") ?? snap.disks[0];

  return (
    <div className="page">
      <div className="page-title">系统状态</div>
      <div className="page-subtitle">
        {snap.host} · {snap.hardware.cpu_model} · {snap.hardware.total_ram} 内存 · {snap.os_version}
      </div>

      <div className="grid-2">
        <div className="panel">
          <div className="panel-title">健康评分</div>
          <div style={{ display: "flex", alignItems: "baseline", gap: 12 }}>
            <span
              className="big-number"
              style={{
                color:
                  snap.health_score >= 90 ? "var(--green)"
                  : snap.health_score >= 60 ? "var(--yellow)"
                  : "var(--red)",
              }}
            >
              {snap.health_score}
            </span>
            <span style={{ color: "var(--text-dim)" }}>{snap.health_score_msg}</span>
          </div>
          <div style={{ color: "var(--text-dim)", marginTop: 8, fontSize: 11 }}>
            运行 {snap.uptime} · {snap.procs} 个进程
          </div>
        </div>

        <div className="panel">
          <div className="panel-title">内存</div>
          <div style={{ display: "flex", alignItems: "center" }}>
            <span className="big-number">{snap.memory.used_percent.toFixed(0)}%</span>
            <Bar percent={snap.memory.used_percent} />
          </div>
          <div style={{ color: "var(--text-dim)", marginTop: 8, fontSize: 11 }}>
            {humanBytes(snap.memory.used)} / {humanBytes(snap.memory.total)}
            {snap.memory.swap_total > 0 && ` · 交换 ${humanBytes(snap.memory.swap_used)}`}
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-title">CPU · {snap.cpu.logical_cpu} 核</div>
        <div style={{ display: "flex", alignItems: "center" }}>
          <span className="big-number">{snap.cpu.usage.toFixed(0)}%</span>
          <Bar percent={snap.cpu.usage} />
        </div>
        <div style={{ color: "var(--text-dim)", marginTop: 8, fontSize: 11 }}>
          负载 {snap.cpu.load1.toFixed(2)} / {snap.cpu.load5.toFixed(2)} / {snap.cpu.load15.toFixed(2)}
        </div>
      </div>

      {primaryDisk && (
        <div className="panel">
          <div className="panel-title">
            磁盘 {primaryDisk.external ? "（外置）" : ""} · {primaryDisk.fstype.toUpperCase()}
          </div>
          <div style={{ display: "flex", alignItems: "center" }}>
            <span className="big-number">{primaryDisk.used_percent.toFixed(0)}%</span>
            <Bar percent={primaryDisk.used_percent} />
          </div>
          <div style={{ color: "var(--text-dim)", marginTop: 8, fontSize: 11 }}>
            已用 {humanBytes(primaryDisk.used)} / {humanBytes(primaryDisk.total)} ·
            剩余 {humanBytes(primaryDisk.total - primaryDisk.used)}
          </div>
          {snap.disks.length > 1 && (
            <div style={{ marginTop: 12 }}>
              {snap.disks
                .filter((d) => d !== primaryDisk)
                .map((d) => (
                  <div key={d.mount} className="row">
                    <div className="row-main">
                      <div className="row-name">{d.mount}</div>
                      <div className="row-path">{d.device}</div>
                    </div>
                    <div className="row-size">
                      {d.used_percent.toFixed(0)}% · {humanBytes(d.total)}
                    </div>
                  </div>
                ))}
            </div>
          )}
        </div>
      )}

      {snap.network.length > 0 && (
        <div className="panel">
          <div className="panel-title">网络</div>
          {snap.network.map((n) => (
            <div key={n.name} className="row">
              <div className="row-main">
                <div className="row-name">{n.name}</div>
              </div>
              <div className="row-size">
                ↓ {n.rx_rate_mbs.toFixed(2)} MB/s · ↑ {n.tx_rate_mbs.toFixed(2)} MB/s
              </div>
            </div>
          ))}
        </div>
      )}

      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <button className="button secondary" onClick={refresh} disabled={refreshing}>
          {refreshing ? "采集中…" : "立即刷新"}
        </button>
      </div>
    </div>
  );
}
