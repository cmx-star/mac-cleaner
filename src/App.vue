<template>
  <div class="app">
    <aside class="sidebar">
      <div class="brand">
        <span class="brand-mark">T</span>
        <span class="brand-name">Talpa</span>
        <button class="lang-toggle" title="Language" @click="toggleLocale">
          {{ getLocale() === "zh-CN" ? "EN" : "中" }}
        </button>
      </div>
      <nav class="nav">
        <button
          v-for="tab_def in TABS"
          :key="tab_def.id"
          :class="['nav-item', tab === tab_def.id ? 'active' : '']"
          @click="tab = tab_def.id"
        >
          <span class="nav-icon">{{ tab_def.icon }}</span>
          {{ t(tab_def.key) }}
        </button>
      </nav>
      <div class="sidebar-footer">
        <span
          class="cli-badge ok"
          :title="cli?.path ? `${cli.path} · 原生引擎` : '原生安全引擎已就绪'"
          style="display: inline-flex; align-items: center"
        >
          <span
            style="
              width: 6px;
              height: 6px;
              border-radius: 50%;
              background: var(--green);
              display: inline-block;
              margin-right: 6px;
              box-shadow: 0 0 6px rgba(77, 182, 172, 0.6);
            "
          />
          {{ cli?.version || "原生引擎" }}
        </span>
      </div>
    </aside>
    <main class="content">
      <StatusPage v-if="tab === 'status'" />
      <CleanPage v-else-if="tab === 'clean'" />
      <OptimizePage v-else-if="tab === 'optimize'" />
      <UninstallPage v-else-if="tab === 'uninstall'" />
      <StartupPage v-else-if="tab === 'startup'" />
      <AnalyzePage v-else-if="tab === 'analyze'" />
      <HistoryPage v-else-if="tab === 'history'" />
    </main>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { cliStatus } from "./api";
import { useT, toggleLocale, getLocale } from "./i18n";
import type { CliLocation } from "./types";
import StatusPage from "./pages/StatusPage.vue";
import CleanPage from "./pages/CleanPage.vue";
import OptimizePage from "./pages/OptimizePage.vue";
import UninstallPage from "./pages/UninstallPage.vue";
import StartupPage from "./pages/StartupPage.vue";
import AnalyzePage from "./pages/AnalyzePage.vue";
import HistoryPage from "./pages/HistoryPage.vue";

type Tab = "status" | "clean" | "optimize" | "uninstall" | "startup" | "analyze" | "history";

const TABS: {
  id: Tab;
  icon: string;
  key: "nav.status" | "nav.clean" | "nav.optimize" | "nav.uninstall" | "nav.startup" | "nav.analyze" | "nav.history";
}[] = [
  { id: "status", icon: "◉", key: "nav.status" },
  { id: "clean", icon: "✦", key: "nav.clean" },
  { id: "optimize", icon: "⚡", key: "nav.optimize" },
  { id: "uninstall", icon: "◎", key: "nav.uninstall" },
  { id: "startup", icon: "⏻", key: "nav.startup" },
  { id: "analyze", icon: "▤", key: "nav.analyze" },
  { id: "history", icon: "⏱", key: "nav.history" },
];

const tab = ref<Tab>("status");
const cli = ref<CliLocation | null>(null);
const t = useT();

onMounted(() => {
  cliStatus()
    .then((c) => (cli.value = c))
    .catch(() => (cli.value = null));
});
</script>
