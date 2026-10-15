/** 轻量 i18n：检测系统语言，提供 t() 与语言切换。 */
import { useSyncExternalStore } from "react";
import { messages, type Locale, type MessageKey } from "./messages";

const STORE_KEY = "talpa.locale";

function detectLocale(): Locale {
  try {
    const saved = localStorage.getItem(STORE_KEY);
    if (saved === "zh-CN" || saved === "en") return saved;
  } catch {
    // localStorage 不可用时走系统语言。
  }
  const langs = navigator.languages ?? [navigator.language];
  for (const l of langs) {
    if (l.toLowerCase().startsWith("zh")) return "zh-CN";
    if (l.toLowerCase().startsWith("en")) return "en";
  }
  return "zh-CN";
}

let current: Locale = detectLocale();
const listeners = new Set<() => void>();

export function getLocale(): Locale {
  return current;
}

export function setLocale(locale: Locale) {
  current = locale;
  try {
    localStorage.setItem(STORE_KEY, locale);
  } catch {
    // 忽略持久化失败。
  }
  listeners.forEach((fn) => fn());
}

export function toggleLocale() {
  setLocale(current === "zh-CN" ? "en" : "zh-CN");
}

function subscribe(fn: () => void) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** React 绑定：语言变化时触发重渲染。 */
export function useLocale(): Locale {
  return useSyncExternalStore(subscribe, getLocale);
}

/** 取文案并做 {name} 插值。非 React 场景也可用。 */
export function translate(key: MessageKey, params?: Record<string, string | number>): string {
  const table = messages[current] as Record<string, string>;
  let text = table[key] ?? (messages["zh-CN"] as Record<string, string>)[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      text = text.replaceAll(`{${k}}`, String(v));
    }
  }
  return text;
}

/** 组件内 hook：语言切换时自动重渲染。 */
export function useT() {
  useLocale();
  return translate;
}
