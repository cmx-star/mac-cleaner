/** 美观的加载组件：spinner + 进度态 + 步骤文案，替代纯文字 pulse。 */
import { useEffect, useRef, useState } from "react";

/** 单圈 spinner + 文案。用于无法估算进度的场合。 */
export function Spinner({ label }: { label?: string }) {
  return (
    <div className="loading-block" role="status" aria-live="polite">
      <span className="spinner" aria-hidden="true" />
      {label && <span className="loading-label">{label}</span>}
    </div>
  );
}

export interface ProgressPhase {
  /** 主步骤名，如「正在扫描可清理项」。 */
  step: string;
  /** 细节文案，如「正在检查 ~/Library/Caches」。 */
  detail?: string;
  /** 0-100。不传则显示流动条纹的「进行中」状态。 */
  percent?: number;
}

/**
 * 扫描进度组件。
 *
 * 后端命令是单次 invoke、无法回报真实进度，所以这里用两段式：
 * 有 percent 显示进度条，没有则显示循环推进的阶段文案 + 流动条纹，
 * 视觉上有「在动」的真实感，而不是一条永远走不完的假进度条。
 */
export function ScanProgress({ phases, active }: { phases: ProgressPhase[]; active: number }) {
  const phase = phases[Math.min(active, phases.length - 1)] ?? phases[0];
  return (
    <div className="loading-block" role="status" aria-live="polite">
      <span className="spinner" aria-hidden="true" />
      <div className="loading-body">
        <div className="loading-label">{phase?.step}</div>
        {phase?.detail && <div className="loading-detail">{phase.detail}</div>}
        {phase?.percent !== undefined ? (
          <div className="progress-track">
            <div className="progress-fill" style={{ width: `${Math.min(phase.percent, 100)}%` }} />
          </div>
        ) : (
          <div className="progress-track indeterminate">
            <div className="progress-stripes" />
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * 逐个扫过目录名的细节文案：前端按已知目录序列轮播，
 * 扫描结束后由父组件切换到下一阶段。
 */
export function useRotatingDetail(dirs: string[], intervalMs = 700): string | null {
  const [index, setIndex] = useState(0);
  const count = dirs.length;
  useEffect(() => {
    if (count === 0) return;
    const timer = setInterval(() => setIndex((i) => (i + 1) % count), intervalMs);
    return () => clearInterval(timer);
  }, [count, intervalMs]);
  return count > 0 ? dirs[index % count] : null;
}

/** 行内小 spinner，用于按钮内或行级操作。 */
export function InlineSpinner() {
  return <span className="spinner inline" aria-hidden="true" />;
}

/** 引用最新回调的小工具，避免轮播计时器闭包过期。 */
export function useLatest<T>(value: T) {
  const ref = useRef(value);
  ref.current = value;
  return ref;
}
