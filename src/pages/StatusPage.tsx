import { useCallback, useEffect, useRef, useState } from "react";
import {
  fanStatus,
  humanBytes,
  killProcess,
  processDetail,
  revealInFinder,
  setFan,
  systemStatus,
  toggleLowLatency,
  topProcesses,
} from "../api";
import { useT } from "../i18n";
import { Spinner, InlineSpinner } from "../components/Loading";
import type { Fan, FanMode, ProcessDetail, ProcessInfo, Snapshot } from "../types";

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

/** 温度走势波形：绘制 CPU 与 GPU 实时动态历史曲线，随温阶渐变着色。 */
function TempSparkline({
  series,
  currentCpu,
  currentGpu,
  height = 68,
}: {
  series: { cpu: number; gpu: number }[];
  currentCpu: number;
  currentGpu: number;
  height?: number;
}) {
  const points =
    series.length >= 2
      ? series
      : [
          { cpu: currentCpu, gpu: currentGpu },
          { cpu: currentCpu, gpu: currentGpu },
        ];

  const allCpu = points.map((p) => p.cpu).filter((v) => v > 0);
  const allGpu = points.map((p) => p.gpu).filter((v) => v > 0);
  const allTemps = [...allCpu, ...allGpu];

  const minVal = allTemps.length > 0 ? Math.min(...allTemps) : 40;
  const maxVal = allTemps.length > 0 ? Math.max(...allTemps) : 80;

  // Y 轴自适应区间，保持适当张力与平滑度，至少 14°C 跨度
  const pad = Math.max((maxVal - minVal) * 0.2, 4);
  const yMin = Math.max(minVal - pad, 20);
  const yMax = maxVal + pad;
  const yRange = Math.max(yMax - yMin, 14);

  const w = 100;
  const h = height;
  const step = w / (points.length - 1);

  const getY = (v: number) => {
    const clamped = Math.min(Math.max(v, yMin), yMax);
    return (h - ((clamped - yMin) / yRange) * (h - 10) - 5).toFixed(2);
  };

  const cpuPoints = points
    .map((p, i) => `${(i * step).toFixed(2)},${getY(p.cpu || currentCpu)}`)
    .join(" ");

  const hasGpu = currentGpu > 0;
  const gpuPoints = hasGpu
    ? points
        .map((p, i) => `${(i * step).toFixed(2)},${getY(p.gpu || currentGpu)}`)
        .join(" ")
    : null;

  // 主折线颜色根据当前 CPU 温度动态变色
  const cpuColor =
    currentCpu > 85 ? "var(--red)" : currentCpu > 70 ? "var(--yellow)" : "var(--green)";

  // CPU 区域填充多边形（渐变半透明柔光）
  const areaPoints = `0,${h} ${cpuPoints} ${(points.length - 1) * step},${h}`;

  return (
    <div style={{ position: "relative", width: "100%", height, flex: 1, minHeight: height }}>
      <svg className="sparkline" viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" style={{ width: "100%", height: "100%", display: "block" }}>
        <defs>
          <linearGradient id="cpuTempAreaGrad" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={cpuColor} stopOpacity="0.28" />
            <stop offset="100%" stopColor={cpuColor} stopOpacity="0.0" />
          </linearGradient>
        </defs>

        {/* CPU 面积柔光 */}
        <polygon points={areaPoints} fill="url(#cpuTempAreaGrad)" />

        {/* GPU 走势曲线（次要，冷色调天蓝细虚线） */}
        {hasGpu && gpuPoints && (
          <polyline
            points={gpuPoints}
            fill="none"
            stroke="#61afef"
            strokeWidth="1.2"
            strokeDasharray="3 2"
            opacity="0.85"
            vectorEffect="non-scaling-stroke"
          />
        )}

        {/* CPU 走势曲线（主曲线，鲜明实线） */}
        <polyline
          points={cpuPoints}
          fill="none"
          stroke={cpuColor}
          strokeWidth="1.6"
          strokeLinecap="round"
          strokeLinejoin="round"
          vectorEffect="non-scaling-stroke"
        />
      </svg>
    </div>
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

/** 为什么在运行详情弹窗。 */
function ProcessDetailModal({
  pid,
  onClose,
  onKilled,
}: {
  pid: number;
  onClose: () => void;
  onKilled: () => void;
}) {
  const [detail, setDetail] = useState<ProcessDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => {
    setLoading(true);
    processDetail(pid)
      .then((d) => {
        setDetail(d);
        setError(null);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [pid]);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [onClose]);

  const handleCopySummary = () => {
    if (!detail) return;
    const summary = [
      `进程: ${detail.name} (PID: ${detail.pid})`,
      `依据: ${detail.basis}`,
      `用户: ${detail.user}`,
      `启动时间: ${detail.start_time_human}`,
      `线程数: ${detail.threads}`,
      `打开文件: ${detail.open_files}`,
      `磁盘 I/O: ${humanBytes(detail.disk_read_bytes)} R · ${humanBytes(detail.disk_write_bytes)} W`,
      `监听端口: ${detail.listening_ports.join(", ") || "未监听端口"}`,
      detail.launchd_service ? `launchd 服务: ${detail.launchd_service}` : "",
      detail.cwd ? `工作目录: ${detail.cwd}` : "",
      detail.exe ? `可执行文件: ${detail.exe}` : "",
      detail.command ? `命令: ${detail.command}` : "",
    ]
      .filter(Boolean)
      .join("\n");
    navigator.clipboard.writeText(summary);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleReveal = async () => {
    if (!detail) return;
    const target = detail.bundle_path || detail.exe;
    if (target) {
      try {
        await revealInFinder(target);
      } catch (e) {
        setMsg(String(e));
      }
    }
  };

  const handleKill = async (force: boolean) => {
    if (!detail) return;
    setBusy(true);
    try {
      await killProcess(detail.pid, force);
      onKilled();
      onClose();
    } catch (e) {
      setMsg(String(e));
      setBusy(false);
    }
  };

  const formatPorts = (ports: string[]) => {
    if (!ports || ports.length === 0) return "未监听端口";
    if (ports.length <= 2) return ports.join(", ");
    return `${ports.slice(0, 2).join(", ")} +${ports.length - 2}`;
  };

  return (
    <div
      className="modal-backdrop"
      onClick={onClose}
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0, 0, 0, 0.72)",
        backdropFilter: "blur(20px)",
        WebkitBackdropFilter: "blur(20px)",
        zIndex: 1000,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 24,
      }}
    >
      <div
        className="proc-detail-modal"
        onClick={(e) => e.stopPropagation()}
        style={{
          width: "100%",
          maxWidth: 620,
          maxHeight: "88vh",
          background: "rgba(28, 30, 29, 0.98)",
          border: "1px solid rgba(255, 255, 255, 0.12)",
          borderRadius: 16,
          boxShadow: "0 28px 72px rgba(0, 0, 0, 0.85)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        }}
      >
        {/* 头部 */}
        <div
          style={{
            padding: "20px 24px 14px",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
            {detail?.icon_png_base64 ? (
              <img
                src={`data:image/png;base64,${detail.icon_png_base64}`}
                style={{ width: 42, height: 42, borderRadius: 10, flexShrink: 0 }}
                alt=""
              />
            ) : (
              <div
                style={{
                  width: 42,
                  height: 42,
                  borderRadius: 10,
                  background: "rgba(255, 255, 255, 0.08)",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  fontSize: 18,
                  fontWeight: 700,
                  flexShrink: 0,
                }}
              >
                {detail?.name.slice(0, 1).toUpperCase() || "?"}
              </div>
            )}
            <div>
              <div style={{ fontSize: 20, fontWeight: 700, color: "var(--text-bright)" }}>
                {detail?.name || (loading ? "正在加载..." : "进程详情")}
              </div>
            </div>
          </div>
          <button
            onClick={onClose}
            style={{
              background: "rgba(255, 255, 255, 0.08)",
              border: "none",
              borderRadius: "50%",
              width: 28,
              height: 28,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              cursor: "pointer",
              color: "var(--text-dim)",
              fontSize: 14,
            }}
          >
            ✕
          </button>
        </div>

        {/* 摘要与父进程关系树 */}
        {detail && (
          <div style={{ padding: "0 24px 14px" }}>
            <div style={{ fontSize: 12, color: "var(--text-dim)", marginBottom: 10 }}>
              {detail.group_summary.total_procs} 个进程 · CPU {detail.group_summary.total_cpu.toFixed(1)}% · MEM {humanBytes(detail.group_summary.total_memory)} · {detail.group_summary.user} · {detail.group_summary.managed_by}
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, color: "var(--text-dim)" }}>
              {detail.parent_tree.map((node, i) => (
                <span key={i} style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
                  {i > 0 && <span style={{ opacity: 0.4 }}>›</span>}
                  <span style={{ color: i === detail.parent_tree.length - 1 ? "var(--text-bright)" : undefined, fontWeight: i === detail.parent_tree.length - 1 ? 600 : 400 }}>
                    {node.name} <span style={{ fontFamily: "monospace", opacity: 0.65, fontSize: 11 }}>{node.pid}</span>
                  </span>
                </span>
              ))}
            </div>
          </div>
        )}

        {/* 详情属性滚动体 */}
        <div style={{ flex: 1, overflowY: "auto", padding: "0 24px 20px" }}>
          {loading ? (
            <div style={{ padding: 40, textAlign: "center", color: "var(--text-dim)" }}>
              <InlineSpinner />
              <div style={{ marginTop: 10, fontSize: 12 }}>正在检索进程运行状态...</div>
            </div>
          ) : error ? (
            <div style={{ padding: 30, color: "var(--red)", fontSize: 13 }}>{error}</div>
          ) : detail ? (
            <>
              <div style={{ display: "flex", flexDirection: "column", gap: 10, fontSize: 13, borderTop: "1px solid rgba(255, 255, 255, 0.08)", paddingTop: 16 }}>
                {[
                  { label: "置信度", value: <span style={{ color: "var(--green)" }}>{detail.confidence}</span> },
                  { label: "识别依据", value: detail.basis, mono: true },
                  { label: "launchd 触发原因", value: detail.launchd_trigger },
                  detail.launchd_service ? { label: "launchd 服务", value: detail.launchd_service, mono: true } : null,
                  { label: "线程数", value: detail.threads, mono: true },
                  { label: "打开文件", value: detail.open_files, mono: true },
                  { label: "磁盘 I/O", value: `${humanBytes(detail.disk_read_bytes)} R · ${humanBytes(detail.disk_write_bytes)} W`, mono: true },
                  { label: "监听端口", value: formatPorts(detail.listening_ports), mono: true },
                  { label: "用户", value: detail.user },
                  { label: "启动时间", value: detail.start_time_human },
                  detail.cwd ? { label: "工作目录", value: detail.cwd, mono: true } : null,
                  detail.exe ? { label: "可执行文件", value: detail.exe, mono: true } : null,
                  detail.command ? { label: "命令", value: detail.command, mono: true } : null,
                ]
                  .filter(Boolean)
                  .map((row, i) => (
                    <div key={i} style={{ display: "grid", gridTemplateColumns: "130px 1fr", gap: 14, alignItems: "baseline" }}>
                      <span style={{ color: "var(--text-dim)", textAlign: "right", fontSize: 12, flexShrink: 0 }}>
                        {row!.label}
                      </span>
                      <span style={{ color: "var(--text-bright)", wordBreak: "break-all", fontFamily: row!.mono ? "monospace" : undefined, fontSize: row!.mono ? 12 : 13 }}>
                        {row!.value}
                      </span>
                    </div>
                  ))}
              </div>

              {/* 关联子进程 */}
              {detail.related_processes.length > 0 && (
                <div style={{ marginTop: 24, borderTop: "1px solid rgba(255, 255, 255, 0.08)", paddingTop: 16 }}>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10, fontSize: 12, color: "var(--text-dim)" }}>
                    <span style={{ fontWeight: 600 }}>进程</span>
                    <span>按内存</span>
                  </div>
                  <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                    <div style={{ display: "grid", gridTemplateColumns: "1fr 70px 60px 80px", alignItems: "center", fontSize: 12, padding: "5px 8px", background: "rgba(255, 255, 255, 0.03)", borderRadius: 6 }}>
                      <span style={{ fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{detail.name}</span>
                      <span style={{ color: "var(--text-dim)", fontFamily: "monospace" }}>{detail.pid}</span>
                      <span style={{ color: "var(--text-dim)", fontFamily: "monospace" }}>--</span>
                      <span style={{ textAlign: "right", fontWeight: 600, fontFamily: "monospace" }}>{humanBytes(detail.group_summary.total_memory)}</span>
                    </div>

                    {detail.related_processes.slice(0, 6).map((rp) => (
                      <div key={rp.pid} style={{ display: "grid", gridTemplateColumns: "1fr 70px 60px 80px", alignItems: "center", fontSize: 12, padding: "4px 8px" }}>
                        <span style={{ color: "var(--text-dim)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{rp.name}</span>
                        <span style={{ color: "var(--text-dim)", fontFamily: "monospace" }}>{rp.pid}</span>
                        <span style={{ color: "var(--text-dim)", fontFamily: "monospace" }}>{rp.cpu_percent.toFixed(1)}%</span>
                        <span style={{ textAlign: "right", color: "var(--text-bright)", fontFamily: "monospace" }}>{humanBytes(rp.memory)}</span>
                      </div>
                    ))}

                    {detail.related_processes.length > 6 && (
                      <div style={{ fontSize: 11, color: "var(--text-dim)", padding: "4px 8px", opacity: 0.7 }}>
                        其余 {detail.related_processes.length - 6} 个进程
                      </div>
                    )}
                  </div>
                </div>
              )}
            </>
          ) : null}
        </div>

        {/* 底部按钮栏 */}
        <div
          style={{
            padding: "14px 24px",
            borderTop: "1px solid rgba(255, 255, 255, 0.08)",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            background: "rgba(20, 22, 21, 0.6)",
          }}
        >
          <div style={{ fontSize: 12, color: "var(--red)" }}>{msg}</div>
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <button className="button secondary" onClick={handleCopySummary} disabled={!detail || busy} style={{ fontSize: 12, padding: "6px 14px" }}>
              {copied ? "已复制" : "复制摘要"}
            </button>
            {(detail?.bundle_path || detail?.exe) && (
              <button className="button secondary" onClick={handleReveal} disabled={busy} style={{ fontSize: 12, padding: "6px 14px" }}>
                在访达中显示
              </button>
            )}
            <button className="button secondary" onClick={() => handleKill(false)} disabled={!detail || busy} style={{ fontSize: 12, padding: "6px 14px" }}>
              终止
            </button>
            <button className="button danger" onClick={() => handleKill(true)} disabled={!detail || busy} style={{ fontSize: 12, padding: "6px 14px" }}>
              强制退出
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export default function StatusPage() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [fans, setFans] = useState<Fan[]>([]);
  const [procs, setProcs] = useState<ProcessInfo[]>([]);
  const [fanBusy, setFanBusy] = useState(false);
  const [fanError, setFanError] = useState<string | null>(null);
  const [showNetPopover, setShowNetPopover] = useState(false);
  const [copied, setCopied] = useState(false);
  const [latencyBusy, setLatencyBusy] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);
  const chipRef = useRef<HTMLSpanElement>(null);

  // 进程详情弹窗与操作菜单状态
  const [detailPid, setDetailPid] = useState<number | null>(null);
  const [menuPid, setMenuPid] = useState<number | null>(null);
  const [pinnedPids, setPinnedPids] = useState<number[]>(() => {
    try {
      return JSON.parse(localStorage.getItem("mole_pinned_pids") || "[]");
    } catch {
      return [];
    }
  });
  const [toastMsg, setToastMsg] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const t = useT();

  const showToast = useCallback((msg: string) => {
    setToastMsg(msg);
    setTimeout(() => {
      setToastMsg((prev) => (prev === msg ? null : prev));
    }, 2200);
  }, []);

  // 固定/取消固定行
  const togglePin = useCallback((pid: number) => {
    setPinnedPids((prev) => {
      const next = prev.includes(pid) ? prev.filter((id) => id !== pid) : [...prev, pid];
      try {
        localStorage.setItem("mole_pinned_pids", JSON.stringify(next));
      } catch {}
      return next;
    });
    setMenuPid(null);
  }, []);

  // 在访达中定位应用
  const handleRevealProc = useCallback(async (p: ProcessInfo) => {
    setMenuPid(null);
    let target = p.bundle_path || p.exe;
    if (!target) {
      try {
        const d = await processDetail(p.pid);
        target = d.bundle_path || d.exe;
      } catch {}
    }
    if (target) {
      try {
        await revealInFinder(target);
      } catch (e) {
        showToast(`定位失败: ${e}`);
      }
    } else {
      showToast("未找到该进程的可执行文件路径");
    }
  }, [showToast]);

  // 点击浮窗外部时自动收起，避免常驻遮挡
  useEffect(() => {
    if (!showNetPopover) return;
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        popoverRef.current &&
        !popoverRef.current.contains(target) &&
        chipRef.current &&
        !chipRef.current.contains(target)
      ) {
        setShowNetPopover(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [showNetPopover]);

  // 进程菜单外部点击与 ESC 监听收起
  useEffect(() => {
    if (menuPid === null) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuPid(null);
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setMenuPid(null);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [menuPid]);

  // 网络速率历史，用于画波形。只保留最近 40 次采样（5 秒一次约 3 分钟）。
  const netHistory = useRef<number[]>([]);
  const [netSeries, setNetSeries] = useState<number[]>([]);

  // 温度历史走势，用于画双折线波形。
  const tempHistory = useRef<{ cpu: number; gpu: number }[]>([]);
  const [tempSeries, setTempSeries] = useState<{ cpu: number; gpu: number }[]>([]);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      const next = await systemStatus();
      setSnap(next);
      setError(null);

      const total = next.network
        .filter((n) => !/^(lo|stf|gif)/.test(n.name))
        .reduce((s, n) => s + n.rx_rate_mbs + n.tx_rate_mbs, 0);
      netHistory.current = [...netHistory.current, total].slice(-40);
      setNetSeries(netHistory.current);

      const curCpu = next.thermal.cpu_temp;
      const curGpu = next.thermal.gpu_temp;
      if (curCpu > 0 || curGpu > 0) {
        tempHistory.current = [...tempHistory.current, { cpu: curCpu, gpu: curGpu }].slice(-40);
        setTempSeries(tempHistory.current);
      }

      // 风扇与进程表独立加载，失败不影响主面板。
      fanStatus().then(setFans).catch(() => setFans([]));
      topProcesses(12).then(setProcs).catch(() => setProcs([]));
    } catch (e) {
      setError(String(e));
    } finally {
      setRefreshing(false);
    }
  }, []);

  // 终止或强制退出进程
  const handleKillProc = useCallback(
    async (pid: number, force: boolean) => {
      setMenuPid(null);
      try {
        await killProcess(pid, force);
        showToast(force ? "已强制退出进程" : "已发送终止信号");
        refresh();
      } catch (e) {
        showToast(`操作失败: ${e}`);
      }
    },
    [refresh, showToast]
  );

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

  const handleToggleLowLatency = async () => {
    if (!snap?.network_detail) return;
    setLatencyBusy(true);
    try {
      await toggleLowLatency(!snap.network_detail.low_latency_enabled);
      const next = await systemStatus();
      setSnap(next);
    } catch (e) {
      console.error(e);
    } finally {
      setLatencyBusy(false);
    }
  };

  if (error) return <div className="error-box">{error}</div>;
  if (!snap) return <Spinner label={t("status.scanning")} />;

  const primaryDisk = snap.disks.find((d) => d.mount === "/") ?? snap.disks[0];
  const cpuTemp = snap.thermal.cpu_temp;
  const gpuTemp = snap.thermal.gpu_temp;
  const peakCpu = tempSeries.reduce(
    (max, p) => Math.max(max, p.cpu),
    cpuTemp
  );
  const peakGpu = tempSeries.reduce(
    (max, p) => Math.max(max, p.gpu),
    gpuTemp
  );
  const fan = fans[0];
  const fanPercent = fan && fan.max_rpm > 0 ? Math.round((fan.rpm / fan.max_rpm) * 100) : null;
  const memPressure = memoryPressure(snap.memory);
  // 中间档转速设为 2500 RPM 左右（限制在风扇合法范围内）
  const midRpm = fan && fan.max_rpm > 0
    ? Math.min(Math.max(2500, Math.round(fan.min_rpm)), Math.round(fan.max_rpm))
    : 2500;

  // 优先选取当前有流量的物理网络接口（如 en0/en1），无流量则优先选 en 系列，避免选中 stf0/lo0 等虚拟接口
  const primaryNet =
    snap.network.find((n) => /^(en|wl)/.test(n.name) && (n.rx_rate_mbs > 0 || n.tx_rate_mbs > 0)) ||
    snap.network.find((n) => /^(en|wl)/.test(n.name)) ||
    snap.network.find((n) => !/^(lo|stf|gif|utun|anpi|awdl|llw|bridge)/.test(n.name)) ||
    snap.network[0];
  const rxRate = primaryNet ? primaryNet.rx_rate_mbs : 0;
  const txRate = primaryNet ? primaryNet.tx_rate_mbs : 0;
  const netName = primaryNet?.name ?? "";
  // utun/lo/stf/gif 等是虚拟接口，标成 Wi-Fi 会误导
  const isWifi = /^(en|wl)/.test(netName) && !/^(utun|lo|stf|gif|bridge|anpi|awdl)/.test(netName);

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
              {snap.gpu ? snap.gpu.usage.toFixed(0) : "0"}
              <span className="unit">%</span>
            </div>
            <Gauge percent={snap.gpu ? snap.gpu.usage : 0} />
            <div className="stat-foot">
              {snap.gpu && snap.gpu.memory_used > 0
                ? `${t("status.gpuFoot")} · ${humanBytes(snap.gpu.memory_used)}`
                : t("status.gpuFoot")}
            </div>
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
            <span className="label" style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--red)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M14 14.76V3.5a2.5 2.5 0 0 0-5 0v11.26a4.5 4.5 0 1 0 5 0z" />
              </svg>
              <span>{t("status.thermal")}</span>
            </span>
            <span className="spacer" />
            {cpuTemp > 0 && (
              <span
                className={`stat-chip ${cpuTemp > 90 ? "danger" : cpuTemp > 75 ? "warn" : "ok"}`}
              >
                {cpuTemp.toFixed(0)}°C ·{" "}
                {cpuTemp > 90
                  ? t("status.tempHot")
                  : cpuTemp > 75
                    ? t("status.tempWarm")
                    : t("status.tempNormal")}
              </span>
            )}
          </div>
          <div className="stat-card-body" style={{ justifyContent: "space-between" }}>
            {cpuTemp > 0 || gpuTemp > 0 ? (
              <>
                <TempSparkline
                  series={tempSeries}
                  currentCpu={cpuTemp}
                  currentGpu={gpuTemp}
                  height={68}
                />
                <div
                  className="stat-foot"
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    fontSize: 11,
                  }}
                >
                  <span
                    style={{ display: "inline-flex", alignItems: "center", gap: 5 }}
                    title="CPU 采样最高峰值"
                  >
                    <span
                      style={{
                        width: 8,
                        height: 2,
                        background:
                          cpuTemp > 85 ? "var(--red)" : cpuTemp > 70 ? "var(--yellow)" : "var(--green)",
                        display: "inline-block",
                        borderRadius: 1,
                      }}
                    />
                    <span>CPU {peakCpu > 0 ? `${peakCpu.toFixed(0)}°C` : "--"}</span>
                  </span>

                  <span style={{ fontSize: 10, color: "var(--text-dim)", opacity: 0.75 }}>
                    峰值
                  </span>

                  {gpuTemp > 0 ? (
                    <span
                      style={{ display: "inline-flex", alignItems: "center", gap: 5 }}
                      title="GPU 采样最高峰值"
                    >
                      <span
                        style={{
                          width: 8,
                          height: 2,
                          background: "#61afef",
                          display: "inline-block",
                          borderRadius: 1,
                        }}
                      />
                      <span>GPU {peakGpu > 0 ? `${peakGpu.toFixed(0)}°C` : "--"}</span>
                    </span>
                  ) : (
                    <span style={{ fontSize: 11, color: "var(--text-dim)" }}>--</span>
                  )}
                </div>
              </>
            ) : (
              <>
                <div
                  style={{
                    flex: 1,
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    color: "var(--text-dim)",
                    fontSize: 13,
                  }}
                >
                  {t("status.tempUnavailable")}
                </div>
                <div className="stat-foot">{t("status.thermal")}</div>
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

        <div
          className="stat-card"
          style={{
            position: "relative",
            overflow: showNetPopover ? "visible" : "hidden",
            zIndex: showNetPopover ? 60 : 1,
          }}
        >
          <div className="stat-card-head">
            <span className="label" style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="10" />
                <line x1="2" y1="12" x2="22" y2="12" />
                <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
              </svg>
              <span>{t("status.network")}</span>
            </span>
            <span className="spacer" />
            <span
              ref={chipRef}
              className={`stat-chip ${showNetPopover ? "active" : ""}`}
              style={{
                cursor: "pointer",
                userSelect: "none",
                transition: "all 0.15s ease",
                borderColor: showNetPopover ? "var(--accent)" : undefined,
                color: showNetPopover ? "var(--accent)" : undefined,
              }}
              onClick={(e) => {
                e.stopPropagation();
                setShowNetPopover((v) => !v);
              }}
            >
              {snap.network_detail?.kind || (isWifi ? "Wi-Fi" : netName || "--")}
            </span>
          </div>

          {showNetPopover && (
            <div
              ref={popoverRef}
              className="net-popover"
              style={{
                position: "absolute",
                top: 38,
                right: 12,
                width: 236,
                background: "rgba(30, 32, 31, 0.96)",
                backdropFilter: "blur(20px)",
                WebkitBackdropFilter: "blur(20px)",
                border: "1px solid rgba(255, 255, 255, 0.14)",
                borderRadius: 12,
                padding: "12px 16px",
                boxShadow: "0 14px 36px rgba(0, 0, 0, 0.65)",
                zIndex: 100,
                display: "flex",
                flexDirection: "column",
                gap: 12,
                color: "#fff",
                fontSize: 13,
              }}
            >
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16 }}>
                <span style={{ color: "var(--text-dim)", fontSize: 12, minWidth: 36 }}>IPv4</span>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <span style={{ fontWeight: 600, fontFamily: "monospace", fontSize: 13, letterSpacing: -0.2 }}>
                    {snap.network_detail?.ipv4 || "127.0.0.1"}
                  </span>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      navigator.clipboard.writeText(snap.network_detail?.ipv4 || "");
                      setCopied(true);
                      setTimeout(() => setCopied(false), 2000);
                    }}
                    style={{
                      background: "transparent",
                      border: "none",
                      cursor: "pointer",
                      padding: "2px 4px",
                      borderRadius: 4,
                      color: copied ? "var(--green)" : "var(--text-dim)",
                      display: "inline-flex",
                      alignItems: "center",
                      gap: 4,
                      fontSize: 11,
                      transition: "color 0.15s",
                    }}
                  >
                    {copied ? (
                      <>
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                          <polyline points="20 6 9 17 4 12" />
                        </svg>
                        <span style={{ fontSize: 11 }}>已复制</span>
                      </>
                    ) : (
                      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                        <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                      </svg>
                    )}
                  </button>
                </div>
              </div>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16 }}>
                <span style={{ color: "var(--text-dim)", fontSize: 12, minWidth: 36 }}>代理</span>
                <span style={{ fontWeight: 600, fontSize: 13 }}>{snap.network_detail?.proxy || "直连"}</span>
              </div>
            </div>
          )}

          <div className="stat-card-body">
            <div className="stat-big">
              {rxRate < 1 ? (rxRate * 1024).toFixed(0) : rxRate.toFixed(1)}
              <span className="unit">{rxRate < 1 ? "KB/s" : "MB/s"}</span>
            </div>
            <Sparkline values={netSeries} />
            <div className="stat-foot" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <span>
                {t("status.netFoot", {
                  up:
                    txRate < 1
                      ? `${(txRate * 1024).toFixed(0)} KB/s`
                      : `${txRate.toFixed(1)} MB/s`,
                })}
              </span>
              <button
                onClick={handleToggleLowLatency}
                disabled={latencyBusy}
                style={{
                  background: "transparent",
                  border: "none",
                  cursor: "pointer",
                  padding: 0,
                  fontSize: 11,
                  color: snap.network_detail?.low_latency_enabled ? "var(--accent)" : "var(--text-dim)",
                  transition: "color 0.15s",
                }}
                onMouseEnter={(e) => (e.currentTarget.style.color = "var(--text)")}
                onMouseLeave={(e) =>
                  (e.currentTarget.style.color = snap.network_detail?.low_latency_enabled
                    ? "var(--accent)"
                    : "var(--text-dim)")
                }
              >
                {latencyBusy
                  ? "切换中…"
                  : snap.network_detail?.low_latency_enabled
                    ? "关闭低延迟"
                    : "开启低延迟"}
              </button>
            </div>
          </div>
        </div>

        <div className="stat-card">
          <div className="stat-card-head">
            <span className="label" style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--yellow)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M12 12c-2.8 0-5-2.2-5-5 0-2.2 1.8-4 4-4 3.9 0 6 4.1 4.5 7.5L12 12z" />
                <path d="M12 12c0-2.8 2.2-5 5-5 2.2 0 4 1.8 4 4 0 3.9-4.1 6-7.5 4.5L12 12z" />
                <path d="M12 12c2.8 0 5 2.2 5 5 0 2.2-1.8 4-4 4-3.9 0-6-4.1-4.5-7.5L12 12z" />
                <path d="M12 12c0 2.8-2.2 5-5 5-2.2 0-4-1.8-4-4 0-3.9 4.1-6 7.5-4.5L12 12z" />
                <circle cx="12" cy="12" r="2" fill="currentColor" />
              </svg>
              <span>{t("status.fan")}</span>
            </span>
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
                    className={`fan-mode ${fan.forced && Math.abs(fan.target_rpm - midRpm) <= 300 ? "active" : ""}`}
                    disabled={fanBusy || fan.max_rpm === 0}
                    title={`${midRpm} RPM`}
                    onClick={() => applyFanMode({ manual: midRpm })}
                  >
                    {t("status.fanHold")}
                  </button>
                  <button
                    className={`fan-mode ${fan.forced && fan.target_rpm >= fan.max_rpm - 100 ? "active" : ""}`}
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
        <div className="panel" style={{ position: "relative" }}>
          <div className="panel-title">{t("status.processes", { count: snap.procs })}</div>
          <table className="proc-table">
            <thead>
              <tr>
                <th>{t("status.procName")}</th>
                <th>{t("status.procMemory")}</th>
                <th>{t("status.procCpu")}</th>
                <th>{t("status.procPid")}</th>
                <th style={{ width: 44, textAlign: "center" }}></th>
              </tr>
            </thead>
            <tbody>
              {(() => {
                const sortedProcs = [...procs].sort((a, b) => {
                  const aPin = pinnedPids.includes(a.pid);
                  const bPin = pinnedPids.includes(b.pid);
                  if (aPin && !bPin) return -1;
                  if (!aPin && bPin) return 1;
                  return b.cpu_percent - a.cpu_percent;
                });

                return sortedProcs.map((p, idx) => {
                  const isPinned = pinnedPids.includes(p.pid);
                  const isMenuOpen = menuPid === p.pid;
                  const isHighCpu = p.cpu_percent >= 30;
                  const openUpward = idx >= sortedProcs.length - 4;

                  return (
                    <tr
                      key={p.pid}
                      className={`proc-row ${isPinned ? "is-pinned" : ""}`}
                      onClick={() => setDetailPid(p.pid)}
                      title="点击查看为什么在运行"
                    >
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
                            <span className="app-icon small app-icon-placeholder" aria-hidden="true">
                              {p.name.slice(0, 1).toUpperCase()}
                            </span>
                          )}
                          <span
                            title={p.name}
                            style={{
                              display: "inline-flex",
                              alignItems: "center",
                              gap: 6,
                              overflow: "hidden",
                              textOverflow: "ellipsis",
                              whiteSpace: "nowrap",
                            }}
                          >
                            <span>{p.name}</span>
                            {isPinned && (
                              <span title="已固定在顶部" style={{ fontSize: 11, opacity: 0.85 }}>
                                📌
                              </span>
                            )}
                          </span>
                        </div>
                      </td>
                      <td>{humanBytes(p.memory)}</td>
                      <td>
                        <div
                          style={{
                            display: "inline-flex",
                            alignItems: "center",
                            justifyContent: "flex-end",
                            gap: 5,
                          }}
                        >
                          {isHighCpu && <span style={{ fontSize: 11 }}>🔥</span>}
                          <span
                            style={{
                              color: isHighCpu ? "var(--red)" : undefined,
                              fontWeight: isHighCpu ? 600 : 400,
                            }}
                          >
                            {p.cpu_percent.toFixed(1)}%
                          </span>
                        </div>
                      </td>
                      <td style={{ color: "var(--text-dim)", fontFamily: "monospace" }}>{p.pid}</td>
                      <td
                        style={{ position: "relative", textAlign: "center", width: 44 }}
                        onClick={(e) => e.stopPropagation()}
                      >
                        <button
                          className={`proc-menu-trigger-btn ${isMenuOpen ? "active" : ""}`}
                          onClick={() => setMenuPid(isMenuOpen ? null : p.pid)}
                          title="操作菜单"
                        >
                          •••
                        </button>

                        {isMenuOpen && (
                          <div
                            ref={menuRef}
                            className="proc-dropdown-menu"
                            style={{
                              ...(openUpward
                                ? { bottom: "calc(100% + 4px)" }
                                : { top: "calc(100% + 4px)" }),
                            }}
                          >
                            <button
                              className="proc-menu-item"
                              onClick={() => togglePin(p.pid)}
                            >
                              <svg
                                width="13"
                                height="13"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                strokeWidth="2"
                                strokeLinecap="round"
                                strokeLinejoin="round"
                              >
                                <line x1="12" y1="17" x2="12" y2="22" />
                                <path d="M5 17h14v-2l-2-2V5a2 2 0 0 0-2-2H9a2 2 0 0 0-2 2v8l-2 2v2z" />
                              </svg>
                              <span>{isPinned ? "取消固定" : "固定行"}</span>
                            </button>

                            <button
                              className="proc-menu-item"
                              onClick={() => {
                                setDetailPid(p.pid);
                                setMenuPid(null);
                              }}
                            >
                              <svg
                                width="13"
                                height="13"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                strokeWidth="2"
                                strokeLinecap="round"
                                strokeLinejoin="round"
                              >
                                <path d="M9 18h6" />
                                <path d="M10 22h4" />
                                <path d="M12 2a7 7 0 0 0-7 7c0 2.5 1.5 4.5 3 6h8c1.5-1.5 3-3.5 3-6a7 7 0 0 0-7-7z" />
                              </svg>
                              <span>为什么在运行？</span>
                            </button>

                            <button
                              className="proc-menu-item"
                              onClick={() => handleRevealProc(p)}
                            >
                              <svg
                                width="13"
                                height="13"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                strokeWidth="2"
                                strokeLinecap="round"
                                strokeLinejoin="round"
                              >
                                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                              </svg>
                              <span>在访达中显示</span>
                            </button>

                            <button
                              className="proc-menu-item"
                              onClick={() => {
                                navigator.clipboard.writeText(p.name);
                                showToast(`已复制进程名称: ${p.name}`);
                                setMenuPid(null);
                              }}
                            >
                              <svg
                                width="13"
                                height="13"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                strokeWidth="2"
                                strokeLinecap="round"
                                strokeLinejoin="round"
                              >
                                <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                              </svg>
                              <span>复制进程名称</span>
                            </button>

                            <button
                              className="proc-menu-item"
                              onClick={() => {
                                navigator.clipboard.writeText(String(p.pid));
                                showToast(`已复制 PID: ${p.pid}`);
                                setMenuPid(null);
                              }}
                            >
                              <svg
                                width="13"
                                height="13"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                strokeWidth="2"
                                strokeLinecap="round"
                                strokeLinejoin="round"
                              >
                                <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                              </svg>
                              <span>复制 PID</span>
                            </button>

                            <div className="proc-menu-divider" />

                            <button
                              className="proc-menu-item"
                              onClick={() => handleKillProc(p.pid, false)}
                            >
                              <svg
                                width="13"
                                height="13"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                strokeWidth="2"
                                strokeLinecap="round"
                                strokeLinejoin="round"
                              >
                                <polyline points="9 10 4 15 9 20" />
                                <path d="M20 4v7a4 4 0 0 1-4 4H4" />
                              </svg>
                              <span>终止</span>
                            </button>

                            <button
                              className="proc-menu-item danger"
                              onClick={() => handleKillProc(p.pid, true)}
                            >
                              <svg
                                width="13"
                                height="13"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                strokeWidth="2"
                                strokeLinecap="round"
                                strokeLinejoin="round"
                              >
                                <circle cx="12" cy="12" r="10" />
                                <line x1="15" y1="9" x2="9" y2="15" />
                                <line x1="9" y1="9" x2="15" y2="15" />
                              </svg>
                              <span>强制退出</span>
                            </button>
                          </div>
                        )}
                      </td>
                    </tr>
                  );
                });
              })()}
            </tbody>
          </table>
        </div>
      )}

      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <button className="button secondary" onClick={refresh} disabled={refreshing}>
          {refreshing ? t("common.refreshing") : t("common.refresh")}
        </button>
      </div>

      {/* 为什么在运行？详情弹窗 */}
      {detailPid !== null && (
        <ProcessDetailModal
          pid={detailPid}
          onClose={() => setDetailPid(null)}
          onKilled={refresh}
        />
      )}

      {/* 轻提示 Toast */}
      {toastMsg && <div className="toast-notification">{toastMsg}</div>}
    </div>
  );
}
