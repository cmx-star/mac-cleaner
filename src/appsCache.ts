/**
 * 已安装应用列表的进程内缓存。
 *
 * `list_apps` 要为每个应用批量提取图标，扫描一次有明显耗时。
 * 页面在侧栏切换时会被卸载，组件内的 state 留不住，所以缓存放在模块级：
 * 切回来直接命中缓存。
 *
 * 生命周期就是本次进程：App 退出后缓存自然消失，下次启动会重新扫描；
 * 会话内只有点刷新按钮或卸载成功后才会更新。不做基于时间的自动过期——
 * 定时过期会让「挂着不动再切回来」触发一次用户没要求的扫描。
 */
import { listApps } from "./api";
import type { App } from "./types";

interface CacheEntry {
  apps: App[];
  /** 写入时间，用于界面显示「多久前扫描」。 */
  at: number;
}

let cache: CacheEntry | null = null;
/** 正在进行的扫描。并发调用共用同一次请求，避免重复扫描。 */
let inflight: Promise<App[]> | null = null;

/** 读取缓存，未命中返回 null。 */
export function peekAppsCache(): App[] | null {
  return cache ? cache.apps : null;
}

/** 距上次扫描的毫秒数，未扫描过返回 null。 */
export function appsCacheAge(): number | null {
  return cache ? Date.now() - cache.at : null;
}

/**
 * 取应用列表。`force` 为 true 时忽略缓存重新扫描。
 *
 * 有缓存时直接返回；否则发起扫描，并复用进行中的请求。
 */
export async function loadApps(force = false): Promise<App[]> {
  if (!force) {
    const cached = peekAppsCache();
    if (cached) return cached;
    if (inflight) return inflight;
  }

  const request = listApps()
    .then((apps) => {
      cache = { apps, at: Date.now() };
      return apps;
    })
    .finally(() => {
      // 只清掉自己这一次，避免把后来的请求误清。
      if (inflight === request) inflight = null;
    });

  inflight = request;
  return request;
}

/** 让缓存失效：卸载改变了已安装列表，下次进入页面重新扫描。 */
export function invalidateAppsCache() {
  cache = null;
}
