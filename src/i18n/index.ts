/** 轻量 i18n：检测系统语言，提供 t() 与语言切换。 */
import { ref } from "vue";
import { messages, type Locale, type MessageKey } from "./messages";
export type { Locale, MessageKey };

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

export const currentLocale = ref<Locale>(detectLocale());

export function getLocale(): Locale {
  return currentLocale.value;
}

export function setLocale(locale: Locale) {
  currentLocale.value = locale;
  try {
    localStorage.setItem(STORE_KEY, locale);
  } catch {
    // 忽略持久化失败。
  }
}

export function toggleLocale() {
  setLocale(currentLocale.value === "zh-CN" ? "en" : "zh-CN");
}

/** 取文案并做 {name} 插值。非组件环境也可直接用。 */
export function translate(key: MessageKey, params?: Record<string, string | number>): string {
  const lang = currentLocale.value;
  const table = messages[lang] as Record<string, string>;
  let text = table[key] ?? (messages["zh-CN"] as Record<string, string>)[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      text = text.replaceAll(`{${k}}`, String(v));
    }
  }
  return text;
}

/** Vue 3 composition hook / helper：语言切换时模板自动响应。 */
export function useT() {
  return translate;
}
