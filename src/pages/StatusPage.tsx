import { useCallback, useEffect, useRef, useState } from "react";
import { fanStatus, setFan, systemStatus, topProcesses } from "../api";
import { humanBytes } from "../api";
import { useT } from "../i18n";
import { Spinner, InlineSpinner } from "../components/Loading";
import type { Fan, FanMode, ProcessInfo, Snapshot } from "../types";

/** 每核心占用柱状图。按占用率着色。 */
function CoreBars({ cores }: { cores: number[] }) {
  if (cores.length === 0) return null;
  return (
    <div className="core-bars" title={cores.map((c) => `${c.toFixed(0)}%`).join("  ")}>
      {cores.map((c, i) => (
        <div
          key={i}
          className={`core-bar ${c > 90 ? "danger" : c > 75 ? "warn" : ""}`}
          style={{ height: `${Math.max(c, 3)}%` }}
        />
      ))}
    </div>
  );
}

/** 渐变进度条。percent 为 0..100，越高越偏黄/红。 */
function Gauge({ percent }: { percent: number }) {
  const cls = percent > 85 ? "danger" : percent > 65 ? "warn" : "";
  return (
    <div className="gauge">
      <div
        className={`gauge-fill ${cls}`}
        style={{ width: `${Math.min(Math.max(percent, 0), 100)}%` }}
      />
    </div>
  );
}

/** 网络速率波形：用最近若干次采样画折线。 */
function Sparkline({ values }: { values: number[] }) {
  if (values.length < 2) return <div className="sparkline" />;
  const max = Math.max(...values, 1e-6);
  const w = 100;
  const h = 34;
  const step = w / (values.length - 1);
  const points = values
    .map((v, i) => `${(i * step).toFixed(2)},${(h - (v / max) * h).toFixed(2)}`)
    .join(" ");
  return (
    <svg className="sparkline" viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none">
      <polyline
        points={points}
        fill="none"
        stroke="var(--accent)"
        strokeWidth="1.5"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}

/** 内存压力判定：结合使用率与交换占用。 */
type PressureKey = "pressureHigh" | "pressureMedium" | "pressureNormal";

function memoryPressure(mem: Snapshot["memory"]): { key: PressureKey; cls: string } {
  const swapRatio = mem.swap_total > 0 ? mem.swap_used / mem.swap_total : 0;
  if (mem.used_percent > 90 || swapRatio > 0.5) return { key: "pressureHigh", cls: "danger" };
  if (mem.used_percent > 75 || swapRatio > 0.15) return { key: "pressureMedium", cls: "warn" };
  return { key: "pressureNormal", cls: "ok" };
}

/** 把字节数拆成数值与单位，便于在卡片里分开排版。 */
function splitBytes(bytes: number): { value: string; unit: string } {
  const text = humanBytes(bytes);
  const m = text.match(/^([\d.]+)\s*([A-Z]+)$/);
  return m ? { value: m[1], unit: m[2] } : { value: text, unit: "" };
}

export default function StatusPage() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [fans, setFans] = useState<Fan[]>([]);
  const [procs, setProcs] = useState<ProcessInfo[]>([]);
  const [fanBusy, setFanBusy] = useState(false);
  const [fanError, setFanError] = useState<string | null>(null);
  const t = useT();

  // 网络速率历史，用于画波形。只保留最近 40 次采样（5 秒一次约 3 分钟）。
  const netHistory = useRef<number[]>([]);
  const [netSeries, setNetSeries] = useState<number[]>([]);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      const next = await systemStatus();
      setSnap(next);
      setError(null);

      const total = next.network.reduce((s, n) => s + n.rx_rate_mbs + n.tx_rate_mbs, 0);
      netHistory.current = [...netHistory.current, total].slice(-40);
      setNetSeries(netHistory.current);

      // 风扇与进程表独立加载，失败不影响主面板。
      fanStatus().then(setFans).catch(() => setFans([]));
      topProcesses(12).then(setProcs).catch(() => setProcs([]));
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

  const applyFanMode = async (mode: FanMode) => {
    setFanBusy(true);
    setFanError(null);
    try {
      await setFan(mode);
      setFans(await fanStatus());
    } catch (e) {
      setFanError(String(e));
    } finally {
      setFanBusy(false);
    }
  };

  if (error) return <div className="error-box">{error}</div>;
  if (!snap) return <Spinner label={t("status.scanning")} />;

  const primaryDisk = snap.disks.find((d) => d.mount === "/") ?? snap.disks[0];
  const cpuTemp = snap.thermal.cpu_temp;
  const gpuTemp = snap.thermal.gpu_temp;
  const fan = fans[0];
  const fanPercent = fan && fan.max_rpm > 0 ? Math.round((fan.rpm / fan.max_rpm) * 100) : null;
  const memPressure = memoryPressure(snap.memory);

  const primaryNet = snap.network[0];
  const rxRate = primaryNet ? primaryNet.rx_rate_mbs : 0;
  const txRate = primaryNet ? primaryNet.tx_rate_mbs : 0;
  const netName = primaryNet?.name ?? "";
  // utun/lo 是虚拟接口，标成 Wi-Fi 会误导。
  const isWifi = /^(en|wl)/.test(netName) && !/^(utun|lo)/.test(netName);

  const free = primaryDisk ? splitBytes(primaryDisk.total - primaryDisk.used) : null;

  return (
    <div className="page">
      <div className="title-row">
        <div className="page-title">{t("status.title")}</div>
      </div>
      <div className="page-subtitle">
        {snap.host} · {snap.hardware.cpu_model} · {snap.hardware.total_ram} · {snap.os_version}
      </div>

      {/* 第一行：健康度（占两列）/ CPU / GPU / 内存 */}
      <div className="stat-grid">
        <div className="stat-card">
          <div className="stat-card-head">
            <span className="label">{t("status.health")}</span>
            <span className="spacer" />
            <span className="stat-chip">{snap.hardware.model}</span>
          </div>
          <div className="stat-card-body">
            <div className="stat-big">
              {snap.health_score}
              <span className="unit">{snap.health_score_msg}</span>
            </div>
            <div
              className={`health-orb ${snap.health_score >= 80 ? "ok" : ""}`}
              aria-hidden="true"
            />
            <div className="stat-foot" style={{ whiteSpace: "normal" }}>
              {t("status.uptime", { uptime: snap.uptime, procs: snap.procs })}
            </div>
          </div>
        </div>

        <div className="stat-card">
          <div className="stat-card-head">
            <span className="label">{t("status.cpuLabel")}</span>
            <span className="spacer" />
            {cpuTemp > 0 && <span className="stat-chip">{cpuTemp.toFixed(0)}°C</span>}
          </div>
          <div className="stat-card-body">
            <div className="stat-big">
              {snap.cpu.usage.toFixed(0)}
              <span className="unit">%</span>
            </div>
            <CoreBars cores={snap.cpu.per_core} />
            <div className="stat-foot">
              {t("status.cpuFoot", {
                cores: snap.cpu.logical_cpu,
                load: snap.cpu.load1.toFixed(1),
              })}
            </div>
          </div>
        </div>

        <div className="stat-card">
          <div className="stat-card-head">
            <span className="label">{t("status.gpuLabel")}</span>
            <span className="spacer" />
            {gpuTemp > 0 && <span className="stat-chip">{gpuTemp.toFixed(0)}°C</span>}
          </div>
          <div className="stat-card-body">
            <div className="stat-big">
              {gpuTemp > 0 ? gpuTemp.toFixed(0) : "--"}
              {gpuTemp > 0 && <span className="unit">°C</span>}
            </div>
            <Gauge percent={gpuTemp > 0 ? (gpuTemp / 110) * 100 : 0} />
            <div className="stat-foot">{t("status.gpuFoot")}</div>
          </div>
        </div>

        <div className="stat-card">
          <div className="stat-card-head">
            <span className="label">{t("status.memory")}</span>
            <span className="spacer" />
            <span className={`stat-chip ${memPressure.cls}`}>
              {t(`status.${memPressure.key}`)}
            </span>
          </div>
          <div className="stat-card-body">
            <div className="stat-big">
              {snap.memory.used_percent.toFixed(0)}
              <span className="unit">%</span>
            </div>
            <Gauge percent={snap.memory.used_percent} />
            <div className="stat-foot">
              {humanBytes(snap.memory.used)} / {humanBytes(snap.memory.total)}
              {snap.memory.swap_total > 0 &&
                ` · ${t("status.swap", { size: humanBytes(snap.memory.swap_used) })}`}
            </div>
          </div>
        </div>
      </div>

      {/* 第二行：温度 / 可用空间 / 网络 / 风扇 */}
      <div className="stat-grid">
        <div className="stat-card">
          <div className="stat-card-head">
            <span className="label">{t("status.thermal")}</span>
            <span className="spacer" />
            {cpuTemp > 0 && (
              <span
                className={`stat-chip ${cpuTemp > 90 ? "danger" : cpuTemp > 75 ? "warn" : "ok"}`}
              >
                {cpuTemp > 90
                  ? t("status.tempHot")
                  : cpuTemp > 75
                    ? t("status.tempWarm")
                    : t("status.tempNormal")}
              </span>
            )}
          </div>
          <div className="stat-card-body">
            {cpuTemp > 0 || gpuTemp > 0 ? (
              <>
                <div className="stat-big">
                  {cpuTemp.toFixed(0)}
                  <span className="unit">°C CPU</span>
                  {gpuTemp > 0 && (
                    <>
                      <span style={{ marginLeft: 8 }}>{gpuTemp.toFixed(0)}</span>
                      <span className="unit">°C GPU</span>
                    </>
                  )}
                </div>
                <Gauge percent={(Math.max(cpuTemp, gpuTemp) / 110) * 100} />
                <div className="stat-foot">{snap.hardware.cpu_model}</div>
              </>
            ) : (
              <>
                <div className="stat-big">--</div>
                <Gauge percent={0} />
                <div className="stat-foot">{t("status.tempUnavailable")}</div>
              </>
            )}
          </div>
        </div>

        <div className="stat-card">
          <div className="stat-card-head">
            <span className="label">{t("status.freeSpace")}</span>
            <span className="spacer" />
            {primaryDisk && <span className="stat-chip">{humanBytes(primaryDisk.total)}</span>}
          </div>
          <div className="stat-card-body">
            {primaryDisk && free ? (
              <>
                <div className="stat-big">
                  {free.value}
                  <span className="unit">{free.unit}</span>
                </div>
                <Gauge percent={primaryDisk.used_percent} />
                <div className="stat-foot">
                  {t("status.diskFoot", {
                    used: humanBytes(primaryDisk.used),
                    percent: primaryDisk.used_percent.toFixed(0),
                  })}
                </div>
              </>
            ) : (
              <>
                <div className="stat-big">--</div>
                <Gauge percent={0} />
                <div className="stat-foot">{t("status.noDisk")}</div>
              </>
            )}
          </div>
        </div>

        <div className="stat-card">
          <div className="stat-card-head">
            <span className="label">{t("status.network")}</span>
            <span className="spacer" />
            <span className="stat-chip">{isWifi ? "Wi-Fi" : netName || "--"}</span>
          </div>
          <div className="stat-card-body">
            <div className="stat-big">
              {rxRate < 1 ? (rxRate * 1024).toFixed(0) : rxRate.toFixed(1)}
              <span className="unit">{rxRate < 1 ? "KB/s" : "MB/s"}</span>
            </div>
            <Sparkline values={netSeries} />
            <div className="stat-foot">
              {t("status.netFoot", {
                up:
                  txRate < 1
                    ? `${(txRate * 1024).toFixed(0)} KB/s`
                    : `${txRate.toFixed(1)} MB/s`,
              })}
            </div>
          </div>
        </div>

        <div className="stat-card">
          <div className="stat-card-head">
            <span className="label">{t("status.fan")}</span>
            <span className="spacer" />
            {fanPercent !== null && (
              <span className="stat-chip">{t("status.fanPercent", { percent: fanPercent })}</span>
            )}
          </div>
          <div className="stat-card-body">
            {fan ? (
              <>
                <div className="stat-big">
                  {fan.rpm.toFixed(0)}
                  <span className="unit">RPM</span>
                </div>
                <div className="fan-modes">
                  <button
                    className={`fan-mode ${!fan.forced ? "active" : ""}`}
                    disabled={fanBusy}
                    onClick={() => applyFanMode({ auto: null })}
                  >
                    {t("status.fanAuto")}
                  </button>
                  <button
                    className="fan-mode"
                    disabled={fanBusy || fan.max_rpm === 0}
                    onClick={() => applyFanMode({ manual: Math.round(fan.rpm) })}
                  >
                    {t("status.fanHold")}
                  </button>
                  <button
                    className="fan-mode"
                    disabled={fanBusy || fan.max_rpm === 0}
                    onClick={() => applyFanMode({ manual: Math.round(fan.max_rpm) })}
                  >
                    {t("status.fanMax")}
                  </button>
                  {fanBusy && <InlineSpinner />}
                </div>
                <div className="stat-foot" title={fanError ?? ""}>
                  {fanError
                    ? fanError
                    : fan.min_rpm > 0 && fan.max_rpm > 0
                      ? t("status.fanRange", {
                          min: fan.min_rpm.toFixed(0),
                          max: fan.max_rpm.toFixed(0),
                        })
                      : t("status.fanAutoHint")}
                </div>
              </>
            ) : (
              <>
                <div className="stat-big">--</div>
                <div className="stat-foot">{t("status.fanUnavailable")}</div>
              </>
            )}
          </div>
        </div>
      </div>

      {/* 底部：进程表 */}
      {procs.length > 0 && (
        <div className="panel">
          <div className="panel-title">{t("status.processes", { count: snap.procs })}</div>
          <table className="proc-table">
            <thead>
              <tr>
                <th>{t("status.procName")}</th>
                <th>{t("status.procMemory")}</th>
                <th>{t("status.procCpu")}</th>
                <th>{t("status.procPid")}</th>
              </tr>
            </thead>
            <tbody>
              {procs.map((p) => (
                <tr key={p.pid}>
                  <td>
                    <div className="proc-name">
                      {p.icon_png_base64 ? (
                        <img
                          className="app-icon small"
                          src={`data:image/png;base64,${p.icon_png_base64}`}
                          alt=""
                          aria-hidden="true"
                        />
                      ) : (
                        // 系统守护进程、CLI 工具没有 bundle 图标，用首字母占位。
                        <span className="app-icon small app-icon-placeholder" aria-hidden="true">
                          {p.name.slice(0, 1).toUpperCase()}
                        </span>
                      )}
                      <span title={p.name}>{p.name}</span>
                    </div>
                  </td>
                  <td>{humanBytes(p.memory)}</td>
                  <td>{p.cpu_percent.toFixed(1)}</td>
                  <td style={{ color: "var(--text-dim)" }}>{p.pid}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <button className="button secondary" onClick={refresh} disabled={refreshing}>
          {refreshing ? t("common.refreshing") : t("common.refresh")}
        </button>
      </div>
    </div>
  );
}
