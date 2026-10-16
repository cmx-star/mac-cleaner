<template>
  <div
    class="modal-backdrop"
    style="
      position: fixed;
      inset: 0;
      background: rgba(0, 0, 0, 0.72);
      backdrop-filter: blur(20px);
      -webkit-backdrop-filter: blur(20px);
      z-index: 1000;
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 24px;
    "
    @click="$emit('close')"
  >
    <div
      class="proc-detail-modal"
      style="
        width: 100%;
        max-width: 620px;
        max-height: 88vh;
        background: rgba(28, 30, 29, 0.98);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 16px;
        box-shadow: 0 28px 72px rgba(0, 0, 0, 0.85);
        display: flex;
        flex-direction: column;
        overflow: hidden;
      "
      @click.stop
    >
      <!-- 头部 -->
      <div
        style="
          padding: 20px 24px 14px;
          display: flex;
          align-items: center;
          justify-content: space-between;
        "
      >
        <div style="display: flex; align-items: center; gap: 14px">
          <img
            v-if="detail?.icon_png_base64"
            :src="`data:image/png;base64,${detail.icon_png_base64}`"
            style="width: 42px; height: 42px; border-radius: 10px; flex-shrink: 0"
            alt=""
          />
          <div
            v-else
            style="
              width: 42px;
              height: 42px;
              border-radius: 10px;
              background: rgba(255, 255, 255, 0.08);
              display: flex;
              align-items: center;
              justify-content: center;
              font-size: 18px;
              font-weight: 700;
              flex-shrink: 0;
            "
          >
            {{ detail?.name.slice(0, 1).toUpperCase() || "?" }}
          </div>
          <div>
            <div style="font-size: 20px; font-weight: 700; color: var(--text-bright)">
              {{ detail?.name || (loading ? "正在加载..." : "进程详情") }}
            </div>
          </div>
        </div>
        <button
          style="
            background: rgba(255, 255, 255, 0.08);
            border: none;
            border-radius: 50%;
            width: 28px;
            height: 28px;
            display: flex;
            align-items: center;
            justify-content: center;
            cursor: pointer;
            color: var(--text-dim);
            font-size: 14px;
          "
          @click="$emit('close')"
        >
          ✕
        </button>
      </div>

      <!-- 摘要与父进程关系树 -->
      <div v-if="detail" style="padding: 0 24px 14px">
        <div style="font-size: 12px; color: var(--text-dim); margin-bottom: 10px">
          {{ detail.group_summary.total_procs }} 个进程 · CPU {{ detail.group_summary.total_cpu.toFixed(1) }}% · MEM {{ humanBytes(detail.group_summary.total_memory) }} · {{ detail.group_summary.user }} · {{ detail.group_summary.managed_by }}
        </div>

        <div style="display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-dim)">
          <span v-for="(node, i) in detail.parent_tree" :key="i" style="display: inline-flex; align-items: center; gap: 6px">
            <span v-if="i > 0" style="opacity: 0.4">›</span>
            <span
              :style="{
                color: i === detail.parent_tree.length - 1 ? 'var(--text-bright)' : undefined,
                fontWeight: i === detail.parent_tree.length - 1 ? 600 : 400,
              }"
            >
              {{ node.name }}
              <span style="font-family: monospace; opacity: 0.65; font-size: 11px">{{ node.pid }}</span>
            </span>
          </span>
        </div>
      </div>

      <!-- 详情属性滚动体 -->
      <div style="flex: 1; overflow-y: auto; padding: 0 24px 20px">
        <div v-if="loading" style="padding: 40px; text-align: center; color: var(--text-dim)">
          <InlineSpinner />
          <div style="margin-top: 10px; font-size: 12px">正在检索进程运行状态...</div>
        </div>
        <div v-else-if="error" style="padding: 30px; color: var(--red); font-size: 13px">
          {{ error }}
        </div>
        <template v-else-if="detail">
          <div
            style="
              display: flex;
              flex-direction: column;
              gap: 10px;
              font-size: 13px;
              border-top: 1px solid rgba(255, 255, 255, 0.08);
              padding-top: 16px;
            "
          >
            <div
              v-for="(row, i) in detailAttributes"
              :key="i"
              style="display: grid; grid-template-columns: 130px 1fr; gap: 14px; align-items: baseline"
            >
              <span style="color: var(--text-dim); text-align: right; font-size: 12px; flex-shrink: 0">
                {{ row.label }}
              </span>
              <span
                :style="{
                  color: 'var(--text-bright)',
                  wordBreak: 'break-all',
                  fontFamily: row.mono ? 'monospace' : undefined,
                  fontSize: row.mono ? '12px' : '13px',
                }"
              >
                <span v-if="row.isGreen" style="color: var(--green)">{{ row.value }}</span>
                <template v-else>{{ row.value }}</template>
              </span>
            </div>
          </div>

          <!-- 关联子进程 -->
          <div
            v-if="detail.related_processes.length > 0"
            style="margin-top: 24px; border-top: 1px solid rgba(255, 255, 255, 0.08); padding-top: 16px"
          >
            <div
              style="
                display: flex;
                justify-content: space-between;
                align-items: center;
                margin-bottom: 10px;
                font-size: 12px;
                color: var(--text-dim);
              "
            >
              <span style="font-weight: 600">进程</span>
              <span>按内存</span>
            </div>
            <div style="display: flex; flex-direction: column; gap: 6px">
              <div
                style="
                  display: grid;
                  grid-template-columns: 1fr 70px 60px 80px;
                  align-items: center;
                  font-size: 12px;
                  padding: 5px 8px;
                  background: rgba(255, 255, 255, 0.03);
                  border-radius: 6px;
                "
              >
                <span style="font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">
                  {{ detail.name }}
                </span>
                <span style="color: var(--text-dim); font-family: monospace">{{ detail.pid }}</span>
                <span style="color: var(--text-dim); font-family: monospace">--</span>
                <span style="text-align: right; font-weight: 600; font-family: monospace">
                  {{ humanBytes(detail.group_summary.total_memory) }}
                </span>
              </div>

              <div
                v-for="rp in detail.related_processes.slice(0, 6)"
                :key="rp.pid"
                style="display: grid; grid-template-columns: 1fr 70px 60px 80px; align-items: center; font-size: 12px; padding: 4px 8px"
              >
                <span style="color: var(--text-dim); overflow: hidden; text-overflow: ellipsis; white-space: nowrap">
                  {{ rp.name }}
                </span>
                <span style="color: var(--text-dim); font-family: monospace">{{ rp.pid }}</span>
                <span style="color: var(--text-dim); font-family: monospace">{{ rp.cpu_percent.toFixed(1) }}%</span>
                <span style="text-align: right; color: var(--text-bright); font-family: monospace">
                  {{ humanBytes(rp.memory) }}
                </span>
              </div>

              <div
                v-if="detail.related_processes.length > 6"
                style="font-size: 11px; color: var(--text-dim); padding: 4px 8px; opacity: 0.7"
              >
                其余 {{ detail.related_processes.length - 6 }} 个进程
              </div>
            </div>
          </div>
        </template>
      </div>

      <!-- 底部按钮栏 -->
      <div
        style="
          padding: 14px 24px;
          border-top: 1px solid rgba(255, 255, 255, 0.08);
          display: flex;
          align-items: center;
          justify-content: space-between;
          background: rgba(20, 22, 21, 0.6);
        "
      >
        <div style="font-size: 12px; color: var(--red)">{{ msg }}</div>
        <div style="display: flex; align-items: center; gap: 10px">
          <button
            class="button secondary"
            :disabled="!detail || busy"
            style="font-size: 12px; padding: 6px 14px"
            @click="handleCopySummary"
          >
            {{ copied ? "已复制" : "复制摘要" }}
          </button>
          <button
            v-if="detail?.bundle_path || detail?.exe"
            class="button secondary"
            :disabled="busy"
            style="font-size: 12px; padding: 6px 14px"
            @click="handleReveal"
          >
            在访达中显示
          </button>
          <button
            class="button secondary"
            :disabled="!detail || busy"
            style="font-size: 12px; padding: 6px 14px"
            @click="handleKill(false)"
          >
            终止
          </button>
          <button
            class="button danger"
            :disabled="!detail || busy"
            style="font-size: 12px; padding: 6px 14px"
            @click="handleKill(true)"
          >
            强制退出
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { humanBytes, killProcess, processDetail, revealInFinder } from "../../api";
import { InlineSpinner } from "../../components/Loading";
import type { ProcessDetail } from "../../types";

const props = defineProps<{
  pid: number;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "killed"): void;
}>();

const detail = ref<ProcessDetail | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);
const copied = ref(false);
const busy = ref(false);
const msg = ref<string | null>(null);

function formatPorts(ports: string[]) {
  if (!ports || ports.length === 0) return "未监听端口";
  if (ports.length <= 2) return ports.join(", ");
  return `${ports.slice(0, 2).join(", ")} +${ports.length - 2}`;
}

const detailAttributes = computed(() => {
  if (!detail.value) return [];
  const d = detail.value;
  const list = [
    { label: "置信度", value: d.confidence, isGreen: true },
    { label: "识别依据", value: d.basis, mono: true },
    { label: "launchd 触发原因", value: d.launchd_trigger },
    d.launchd_service ? { label: "launchd 服务", value: d.launchd_service, mono: true } : null,
    { label: "线程数", value: String(d.threads), mono: true },
    { label: "打开文件", value: String(d.open_files), mono: true },
    {
      label: "磁盘 I/O",
      value: `${humanBytes(d.disk_read_bytes)} R · ${humanBytes(d.disk_write_bytes)} W`,
      mono: true,
    },
    { label: "监听端口", value: formatPorts(d.listening_ports), mono: true },
    { label: "用户", value: d.user },
    { label: "启动时间", value: d.start_time_human },
    d.cwd ? { label: "工作目录", value: d.cwd, mono: true } : null,
    d.exe ? { label: "可执行文件", value: d.exe, mono: true } : null,
    d.command ? { label: "命令", value: d.command, mono: true } : null,
  ];
  return list.filter(Boolean) as {
    label: string;
    value: string;
    mono?: boolean;
    isGreen?: boolean;
  }[];
});

onMounted(() => {
  loading.value = true;
  processDetail(props.pid)
    .then((d) => {
      detail.value = d;
      error.value = null;
    })
    .catch((e) => {
      error.value = String(e);
    })
    .finally(() => {
      loading.value = false;
    });

  const handleKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") emit("close");
  };
  window.addEventListener("keydown", handleKey);
  onUnmounted(() => {
    window.removeEventListener("keydown", handleKey);
  });
});

function handleCopySummary() {
  if (!detail.value) return;
  const d = detail.value;
  const summary = [
    `进程: ${d.name} (PID: ${d.pid})`,
    `依据: ${d.basis}`,
    `用户: ${d.user}`,
    `启动时间: ${d.start_time_human}`,
    `线程数: ${d.threads}`,
    `打开文件: ${d.open_files}`,
    `磁盘 I/O: ${humanBytes(d.disk_read_bytes)} R · ${humanBytes(d.disk_write_bytes)} W`,
    `监听端口: ${d.listening_ports.join(", ") || "未监听端口"}`,
    d.launchd_service ? `launchd 服务: ${d.launchd_service}` : "",
    d.cwd ? `工作目录: ${d.cwd}` : "",
    d.exe ? `可执行文件: ${d.exe}` : "",
    d.command ? `命令: ${d.command}` : "",
  ]
    .filter(Boolean)
    .join("\n");
  navigator.clipboard.writeText(summary);
  copied.value = true;
  setTimeout(() => {
    copied.value = false;
  }, 2000);
}

async function handleReveal() {
  if (!detail.value) return;
  const target = detail.value.bundle_path || detail.value.exe;
  if (target) {
    try {
      await revealInFinder(target);
    } catch (e) {
      msg.value = String(e);
    }
  }
}

async function handleKill(force: boolean) {
  if (!detail.value) return;
  busy.value = true;
  try {
    await killProcess(detail.value.pid, force);
    emit("killed");
    emit("close");
  } catch (e) {
    msg.value = String(e);
    busy.value = false;
  }
}
</script>
