<template>
  <div v-if="error" class="error-box">{{ error }}</div>
  <Spinner v-else-if="!snap" :label="t('status.scanning')" />

  <div v-else class="page">
    <div class="title-row">
      <div class="page-title">{{ t("status.title") }}</div>
    </div>
    <div class="page-subtitle">
      {{ snap.host }} · {{ snap.hardware.cpu_model }} · {{ snap.hardware.total_ram }} · {{ snap.os_version }}
    </div>

    <!-- 第一行：健康度 / CPU / GPU / 内存 -->
    <div class="stat-grid">
      <!-- 健康度 -->
      <div class="stat-card">
        <div class="stat-card-head">
          <span class="label">
            <HealthIcon />
            <span>{{ t("status.health") }}</span>
          </span>
          <span class="spacer" />
          <span class="stat-chip">{{ snap.hardware.model }}</span>
        </div>
        <div class="stat-card-body">
          <div class="stat-big">
            {{ snap.health_score }}
            <span class="unit">{{ snap.health_score_msg }}</span>
          </div>
          <div
            :class="['health-orb', snap.health_score >= 80 ? 'ok' : '']"
            aria-hidden="true"
          />
          <div class="stat-foot" style="white-space: normal">
            {{ t("status.uptime", { uptime: snap.uptime, procs: snap.procs }) }}
          </div>
        </div>
      </div>

      <!-- CPU -->
      <div class="stat-card">
        <div class="stat-card-head">
          <span class="label">
            <CpuIcon />
            <span>{{ t("status.cpuLabel") }}</span>
          </span>
          <span class="spacer" />
          <span v-if="cpuTemp > 0" class="stat-chip">{{ cpuTemp.toFixed(0) }}°C</span>
        </div>
        <div class="stat-card-body">
          <div class="stat-big">
            {{ snap.cpu.usage.toFixed(0) }}
            <span class="unit">%</span>
          </div>
          <CoreBars :cores="snap.cpu.per_core" />
          <div class="stat-foot">
            {{
              t("status.cpuFoot", {
                cores: snap.cpu.logical_cpu,
                load: snap.cpu.load1.toFixed(1),
              })
            }}
          </div>
        </div>
      </div>

      <!-- GPU -->
      <div class="stat-card">
        <div class="stat-card-head">
          <span class="label">
            <GpuIcon />
            <span>{{ t("status.gpuLabel") }}</span>
          </span>
          <span class="spacer" />
          <span v-if="gpuTemp > 0" class="stat-chip">{{ gpuTemp.toFixed(0) }}°C</span>
        </div>
        <div class="stat-card-body">
          <div class="stat-big">
            {{ snap.gpu ? snap.gpu.usage.toFixed(0) : "0" }}
            <span class="unit">%</span>
          </div>
          <Gauge :percent="snap.gpu ? snap.gpu.usage : 0" />
          <div class="stat-foot">
            {{
              snap.gpu && snap.gpu.memory_used > 0
                ? `${t("status.gpuFoot")} · ${humanBytes(snap.gpu.memory_used)}`
                : t("status.gpuFoot")
            }}
          </div>
        </div>
      </div>

      <!-- 内存 -->
      <div class="stat-card">
        <div class="stat-card-head">
          <span class="label">
            <MemoryIcon />
            <span>{{ t("status.memory") }}</span>
          </span>
          <span class="spacer" />
          <span :class="['stat-chip', memPressure.cls]">
            {{ t(`status.${memPressure.key}`) }}
          </span>
        </div>
        <div class="stat-card-body">
          <div class="stat-big">
            {{ snap.memory.used_percent.toFixed(0) }}
            <span class="unit">%</span>
          </div>
          <Gauge :percent="snap.memory.used_percent" />
          <div class="stat-foot">
            {{ humanBytes(snap.memory.used) }} / {{ humanBytes(snap.memory.total) }}
            <template v-if="snap.memory.swap_total > 0">
              · {{ t("status.swap", { size: humanBytes(snap.memory.swap_used) }) }}
            </template>
          </div>
        </div>
      </div>
    </div>

    <!-- 第二行：温度 / 可用空间 / 网络 / 风扇 -->
    <div class="stat-grid">
      <!-- 温度 -->
      <div class="stat-card">
        <div class="stat-card-head">
          <span class="label">
            <ThermalIcon />
            <span>{{ t("status.thermal") }}</span>
          </span>
          <span class="spacer" />
          <span
            v-if="cpuTemp > 0"
            :class="['stat-chip', cpuTemp > 90 ? 'danger' : cpuTemp > 75 ? 'warn' : 'ok']"
          >
            {{ cpuTemp.toFixed(0) }}°C ·
            {{
              cpuTemp > 90
                ? t("status.tempHot")
                : cpuTemp > 75
                  ? t("status.tempWarm")
                  : t("status.tempNormal")
            }}
          </span>
        </div>
        <div class="stat-card-body" style="justify-content: space-between">
          <template v-if="cpuTemp > 0 || gpuTemp > 0">
            <TempSparkline
              :series="tempSeries"
              :current-cpu="cpuTemp"
              :current-gpu="gpuTemp"
              :height="68"
            />
            <div
              class="stat-foot"
              style="
                display: flex;
                justify-content: space-between;
                align-items: center;
                font-size: 11px;
              "
            >
              <span
                style="display: inline-flex; align-items: center; gap: 5px"
                title="CPU 采样最高峰值"
              >
                <span
                  :style="{
                    width: '8px',
                    height: '2px',
                    background:
                      cpuTemp > 85 ? 'var(--red)' : cpuTemp > 70 ? 'var(--yellow)' : 'var(--green)',
                    display: 'inline-block',
                    borderRadius: '1px',
                  }"
                />
                <span>CPU {{ peakCpu > 0 ? `${peakCpu.toFixed(0)}°C` : "--" }}</span>
              </span>

              <span style="font-size: 10px; color: var(--text-dim); opacity: 0.75">
                峰值
              </span>

              <span
                v-if="gpuTemp > 0"
                style="display: inline-flex; align-items: center; gap: 5px"
                title="GPU 采样最高峰值"
              >
                <span
                  style="
                    width: 8px;
                    height: 2px;
                    background: #61afef;
                    display: inline-block;
                    border-radius: 1px;
                  "
                />
                <span>GPU {{ peakGpu > 0 ? `${peakGpu.toFixed(0)}°C` : "--" }}</span>
              </span>
              <span v-else style="font-size: 11px; color: var(--text-dim)">--</span>
            </div>
          </template>
          <template v-else>
            <div
              style="
                flex: 1;
                display: flex;
                align-items: center;
                justify-content: center;
                color: var(--text-dim);
                font-size: 13px;
              "
            >
              {{ t("status.tempUnavailable") }}
            </div>
            <div class="stat-foot">{{ t("status.thermal") }}</div>
          </template>
        </div>
      </div>

      <!-- 可用空间 -->
      <div class="stat-card">
        <div class="stat-card-head">
          <span class="label">
            <DiskIcon />
            <span>{{ t("status.freeSpace") }}</span>
          </span>
          <span class="spacer" />
          <span v-if="primaryDisk" class="stat-chip">{{ humanBytes(primaryDisk.total) }}</span>
        </div>
        <div class="stat-card-body">
          <template v-if="primaryDisk && free">
            <div class="stat-big">
              {{ free.value }}
              <span class="unit">{{ free.unit }}</span>
            </div>
            <Gauge :percent="primaryDisk.used_percent" />
            <div class="stat-foot">
              {{
                t("status.diskFoot", {
                  used: humanBytes(primaryDisk.used),
                  percent: primaryDisk.used_percent.toFixed(0),
                })
              }}
            </div>
          </template>
          <template v-else>
            <div class="stat-big">--</div>
            <Gauge :percent="0" />
            <div class="stat-foot">{{ t("status.noDisk") }}</div>
          </template>
        </div>
      </div>

      <!-- 网络 -->
      <div
        class="stat-card"
        :style="{
          position: 'relative',
          overflow: showNetPopover ? 'visible' : 'hidden',
          zIndex: showNetPopover ? 60 : 1,
        }"
      >
        <div class="stat-card-head">
          <span class="label">
            <NetworkIcon :is-wifi="isWifi" />
            <span>{{ t("status.network") }}</span>
          </span>
          <span class="spacer" />
          <span
            ref="chipRef"
            :class="['stat-chip', showNetPopover ? 'active' : '']"
            :style="{
              cursor: 'pointer',
              userSelect: 'none',
              transition: 'all 0.15s ease',
              borderColor: showNetPopover ? 'var(--accent)' : undefined,
              color: showNetPopover ? 'var(--accent)' : undefined,
            }"
            @click.stop="showNetPopover = !showNetPopover"
          >
            {{ snap.network_detail?.kind || (isWifi ? "Wi-Fi" : netName || "--") }}
          </span>
        </div>

        <!-- 悬浮弹窗：IPv4 与代理 -->
        <div
          v-if="showNetPopover"
          ref="popoverRef"
          class="net-popover"
          style="
            position: absolute;
            top: 38px;
            right: 12px;
            width: 236px;
            background: rgba(30, 32, 31, 0.96);
            backdrop-filter: blur(20px);
            -webkit-backdrop-filter: blur(20px);
            border: 1px solid rgba(255, 255, 255, 0.14);
            border-radius: 12px;
            padding: 12px 16px;
            box-shadow: 0 14px 36px rgba(0, 0, 0, 0.65);
            z-index: 100;
            display: flex;
            flex-direction: column;
            gap: 12px;
            color: #fff;
            font-size: 13px;
          "
          @click.stop
        >
          <div style="display: flex; align-items: center; justify-content: space-between; gap: 16px">
            <span style="color: var(--text-dim); font-size: 12px; min-width: 36px">IPv4</span>
            <div style="display: flex; align-items: center; gap: 8px">
              <span style="font-weight: 600; font-family: monospace; font-size: 13px; letter-spacing: -0.2px">
                {{ snap.network_detail?.ipv4 || "127.0.0.1" }}
              </span>
              <button
                style="
                  background: transparent;
                  border: none;
                  cursor: pointer;
                  padding: 2px 4px;
                  border-radius: 4px;
                  display: inline-flex;
                  align-items: center;
                  gap: 4px;
                  font-size: 11px;
                  transition: color 0.15s;
                "
                :style="{ color: copied ? 'var(--green)' : 'var(--text-dim)' }"
                @click.stop="copyIpv4"
              >
                <template v-if="copied">
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                  <span style="font-size: 11px">已复制</span>
                </template>
                <template v-else>
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                  </svg>
                </template>
              </button>
            </div>
          </div>
          <div style="display: flex; align-items: center; justify-content: space-between; gap: 16px">
            <span style="color: var(--text-dim); font-size: 12px; min-width: 36px">代理</span>
            <span style="font-weight: 600; font-size: 13px">{{ snap.network_detail?.proxy || "直连" }}</span>
          </div>
        </div>

        <div class="stat-card-body">
          <div class="stat-big">
            {{ rxRate < 1 ? (rxRate * 1024).toFixed(0) : rxRate.toFixed(1) }}
            <span class="unit">{{ rxRate < 1 ? "KB/s" : "MB/s" }}</span>
          </div>
          <Sparkline :values="netSeries" />
          <div class="stat-foot" style="display: flex; justify-content: space-between; align-items: center">
            <span>
              {{
                t("status.netFoot", {
                  up:
                    txRate < 1
                      ? `${(txRate * 1024).toFixed(0)} KB/s`
                      : `${txRate.toFixed(1)} MB/s`,
                })
              }}
            </span>
            <button
              :disabled="latencyBusy"
              style="
                background: transparent;
                border: none;
                cursor: pointer;
                padding: 0;
                font-size: 11px;
                transition: color 0.15s;
              "
              :style="{
                color: snap.network_detail?.low_latency_enabled ? 'var(--accent)' : 'var(--text-dim)',
              }"
              @click="handleToggleLowLatency"
            >
              {{
                latencyBusy
                  ? "切换中…"
                  : snap.network_detail?.low_latency_enabled
                    ? "关闭低延迟"
                    : "开启低延迟"
              }}
            </button>
          </div>
        </div>
      </div>

      <!-- 风扇 -->
      <div class="stat-card">
        <div class="stat-card-head">
          <span class="label">
            <FanIcon />
            <span>{{ t("status.fan") }}</span>
          </span>
          <span class="spacer" />
          <span v-if="fanPercent !== null" class="stat-chip">
            {{ t("status.fanPercent", { percent: fanPercent }) }}
          </span>
        </div>
        <div class="stat-card-body">
          <template v-if="fan">
            <div class="stat-big">
              {{ fan.rpm.toFixed(0) }}
              <span class="unit">RPM</span>
            </div>
            <div class="fan-modes">
              <button
                :class="['fan-mode', !fan.forced ? 'active' : '']"
                :disabled="fanBusy"
                @click="applyFanMode({ auto: null })"
              >
                {{ t("status.fanAuto") }}
              </button>
              <button
                :class="['fan-mode', fan.forced && Math.abs(fan.target_rpm - midRpm) <= 300 ? 'active' : '']"
                :disabled="fanBusy || fan.max_rpm === 0"
                :title="`${midRpm} RPM`"
                @click="applyFanMode({ manual: midRpm })"
              >
                {{ t("status.fanHold") }}
              </button>
              <button
                :class="['fan-mode', fan.forced && fan.target_rpm >= fan.max_rpm - 100 ? 'active' : '']"
                :disabled="fanBusy || fan.max_rpm === 0"
                @click="applyFanMode({ manual: Math.round(fan.max_rpm) })"
              >
                {{ t("status.fanMax") }}
              </button>
              <InlineSpinner v-if="fanBusy" />
            </div>
            <div class="stat-foot" :title="fanError ?? ''">
              {{
                fanError
                  ? fanError
                  : fan.min_rpm > 0 && fan.max_rpm > 0
                    ? t("status.fanRange", {
                        min: fan.min_rpm.toFixed(0),
                        max: fan.max_rpm.toFixed(0),
                      })
                    : t("status.fanAutoHint")
              }}
            </div>
          </template>
          <template v-else>
            <div class="stat-big">--</div>
            <div class="stat-foot">{{ t("status.fanUnavailable") }}</div>
          </template>
        </div>
      </div>
    </div>

    <!-- 底部：进程表 -->
    <div v-if="procs.length > 0" class="panel" style="position: relative">
      <div class="panel-title">{{ t("status.processes", { count: snap.procs }) }}</div>
      <table class="proc-table">
        <thead>
          <tr>
            <th>{{ t("status.procName") }}</th>
            <th>{{ t("status.procMemory") }}</th>
            <th>{{ t("status.procCpu") }}</th>
            <th>{{ t("status.procPid") }}</th>
            <th style="width: 44px; text-align: center"></th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(p, idx) in sortedProcs"
            :key="p.pid"
            :class="['proc-row', pinnedPids.includes(p.pid) ? 'is-pinned' : '']"
            title="点击查看为什么在运行"
            @click="detailPid = p.pid"
          >
            <td>
              <div class="proc-name">
                <img
                  v-if="p.icon_png_base64"
                  class="app-icon small"
                  :src="`data:image/png;base64,${p.icon_png_base64}`"
                  alt=""
                  aria-hidden="true"
                />
                <span v-else class="app-icon small app-icon-placeholder" aria-hidden="true">
                  {{ p.name.slice(0, 1).toUpperCase() }}
                </span>
                <span
                  :title="p.name"
                  style="
                    display: inline-flex;
                    align-items: center;
                    gap: 6px;
                    overflow: hidden;
                    text-overflow: ellipsis;
                    white-space: nowrap;
                  "
                >
                  <span>{{ p.name }}</span>
                  <span v-if="pinnedPids.includes(p.pid)" title="已固定在顶部" style="font-size: 11px; opacity: 0.85">
                    📌
                  </span>
                </span>
              </div>
            </td>
            <td>{{ humanBytes(p.memory) }}</td>
            <td>
              <div style="display: inline-flex; align-items: center; justify-content: flex-end; gap: 5px">
                <span v-if="p.cpu_percent >= 30" style="font-size: 11px">🔥</span>
                <span
                  :style="{
                    color: p.cpu_percent >= 30 ? 'var(--red)' : undefined,
                    fontWeight: p.cpu_percent >= 30 ? 600 : 400,
                  }"
                >
                  {{ p.cpu_percent.toFixed(1) }}%
                </span>
              </div>
            </td>
            <td style="color: var(--text-dim); font-family: monospace">{{ p.pid }}</td>
            <td style="position: relative; text-align: center; width: 44px" @click.stop>
              <button
                :class="['proc-menu-trigger-btn', menuPid === p.pid ? 'active' : '']"
                title="操作菜单"
                @click="menuPid = menuPid === p.pid ? null : p.pid"
              >
                •••
              </button>

              <!-- 进程操作下拉菜单 -->
              <div
                v-if="menuPid === p.pid"
                ref="menuRef"
                class="proc-dropdown-menu"
                :style="idx >= sortedProcs.length - 4 ? { bottom: 'calc(100% + 4px)' } : { top: 'calc(100% + 4px)' }"
              >
                <button class="proc-menu-item" @click="togglePin(p.pid)">
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <line x1="12" y1="17" x2="12" y2="22" />
                    <path d="M5 17h14v-2l-2-2V5a2 2 0 0 0-2-2H9a2 2 0 0 0-2 2v8l-2 2v2z" />
                  </svg>
                  <span>{{ pinnedPids.includes(p.pid) ? "取消固定" : "固定行" }}</span>
                </button>

                <button
                  class="proc-menu-item"
                  @click="
                    detailPid = p.pid;
                    menuPid = null;
                  "
                >
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M9 18h6" />
                    <path d="M10 22h4" />
                    <path d="M12 2a7 7 0 0 0-7 7c0 2.5 1.5 4.5 3 6h8c1.5-1.5 3-3.5 3-6a7 7 0 0 0-7-7z" />
                  </svg>
                  <span>为什么在运行？</span>
                </button>

                <button class="proc-menu-item" @click="handleRevealProc(p)">
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                  </svg>
                  <span>在访达中显示</span>
                </button>

                <button
                  class="proc-menu-item"
                  @click="
                    copyText(p.name);
                    showToast(`已复制进程名称: ${p.name}`);
                    menuPid = null;
                  "
                >
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                  </svg>
                  <span>复制进程名称</span>
                </button>

                <button
                  class="proc-menu-item"
                  @click="
                    copyText(String(p.pid));
                    showToast(`已复制 PID: ${p.pid}`);
                    menuPid = null;
                  "
                >
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                  </svg>
                  <span>复制 PID</span>
                </button>

                <div class="proc-menu-divider" />

                <button class="proc-menu-item" @click="handleKillProc(p.pid, false)">
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="9 10 4 15 9 20" />
                    <path d="M20 4v7a4 4 0 0 1-4 4H4" />
                  </svg>
                  <span>终止</span>
                </button>

                <button class="proc-menu-item danger" @click="handleKillProc(p.pid, true)">
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <circle cx="12" cy="12" r="10" />
                    <line x1="15" y1="9" x2="9" y2="15" />
                    <line x1="9" y1="9" x2="15" y2="15" />
                  </svg>
                  <span>强制退出</span>
                </button>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div style="display: flex; justify-content: flex-end">
      <button class="button secondary" :disabled="refreshing" @click="refresh">
        {{ refreshing ? t("common.refreshing") : t("common.refresh") }}
      </button>
    </div>

    <!-- 为什么在运行？详情弹窗 -->
    <ProcessDetailModal
      v-if="detailPid !== null"
      :pid="detailPid"
      @close="detailPid = null"
      @killed="refresh"
    />

    <!-- 轻提示 Toast -->
    <div v-if="toastMsg" class="toast-notification">{{ toastMsg }}</div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
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
import {
  HealthIcon,
  CpuIcon,
  GpuIcon,
  MemoryIcon,
  ThermalIcon,
  DiskIcon,
  NetworkIcon,
  FanIcon,
} from "../components/Icons";
import CoreBars from "./status/CoreBars.vue";
import Gauge from "./status/Gauge.vue";
import Sparkline from "./status/Sparkline.vue";
import TempSparkline from "./status/TempSparkline.vue";
import ProcessDetailModal from "./status/ProcessDetailModal.vue";
import type { Fan, FanMode, ProcessInfo, Snapshot } from "../types";

type PressureKey = "pressureHigh" | "pressureMedium" | "pressureNormal";

function memoryPressure(mem: Snapshot["memory"]): { key: PressureKey; cls: string } {
  const swapRatio = mem.swap_total > 0 ? mem.swap_used / mem.swap_total : 0;
  if (mem.used_percent > 90 || swapRatio > 0.5) return { key: "pressureHigh", cls: "danger" };
  if (mem.used_percent > 75 || swapRatio > 0.15) return { key: "pressureMedium", cls: "warn" };
  return { key: "pressureNormal", cls: "ok" };
}

function splitBytes(bytes: number): { value: string; unit: string } {
  const text = humanBytes(bytes);
  const m = text.match(/^([\d.]+)\s*([A-Z]+)$/);
  return m ? { value: m[1], unit: m[2] } : { value: text, unit: "" };
}

const t = useT();
const snap = ref<Snapshot | null>(null);
const error = ref<string | null>(null);
const refreshing = ref(false);
const fans = ref<Fan[]>([]);
const procs = ref<ProcessInfo[]>([]);
const fanBusy = ref(false);
const fanError = ref<string | null>(null);
const showNetPopover = ref(false);
const copied = ref(false);
const latencyBusy = ref(false);
const popoverRef = ref<HTMLDivElement | null>(null);
const chipRef = ref<HTMLSpanElement | null>(null);

const detailPid = ref<number | null>(null);
const menuPid = ref<number | null>(null);
function getInitialPinnedPids(): number[] {
  try {
    return JSON.parse(localStorage.getItem("mole_pinned_pids") || "[]");
  } catch {
    return [];
  }
}
const pinnedPids = ref<number[]>(getInitialPinnedPids());
const toastMsg = ref<string | null>(null);
const menuRef = ref<HTMLDivElement | null>(null);

const netSeries = ref<number[]>([]);
const tempSeries = ref<{ cpu: number; gpu: number }[]>([]);

function showToast(msg: string) {
  toastMsg.value = msg;
  setTimeout(() => {
    if (toastMsg.value === msg) {
      toastMsg.value = null;
    }
  }, 2200);
}

function togglePin(pid: number) {
  const prev = pinnedPids.value;
  const next = prev.includes(pid) ? prev.filter((id) => id !== pid) : [...prev, pid];
  pinnedPids.value = next;
  try {
    localStorage.setItem("mole_pinned_pids", JSON.stringify(next));
  } catch {}
  menuPid.value = null;
}

async function handleRevealProc(p: ProcessInfo) {
  menuPid.value = null;
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
}

async function handleKillProc(pid: number, force: boolean) {
  menuPid.value = null;
  try {
    await killProcess(pid, force);
    showToast(force ? "已强制退出进程" : "已发送终止信号");
    refresh();
  } catch (e) {
    showToast(`操作失败: ${e}`);
  }
}

function copyText(str: string) {
  navigator.clipboard.writeText(str);
}

function copyIpv4() {
  navigator.clipboard.writeText(snap.value?.network_detail?.ipv4 || "");
  copied.value = true;
  setTimeout(() => {
    copied.value = false;
  }, 2000);
}

async function applyFanMode(mode: FanMode) {
  fanBusy.value = true;
  fanError.value = null;
  try {
    await setFan(mode);
    fans.value = await fanStatus();
  } catch (e) {
    fanError.value = String(e);
  } finally {
    fanBusy.value = false;
  }
}

async function handleToggleLowLatency() {
  if (!snap.value?.network_detail) return;
  latencyBusy.value = true;
  try {
    await toggleLowLatency(!snap.value.network_detail.low_latency_enabled);
    const next = await systemStatus();
    snap.value = next;
  } catch (e) {
    console.error(e);
  } finally {
    latencyBusy.value = false;
  }
}

async function refresh() {
  refreshing.value = true;
  try {
    const next = await systemStatus();
    snap.value = next;
    error.value = null;

    const total = next.network
      .filter((n) => !/^(lo|stf|gif)/.test(n.name))
      .reduce((s, n) => s + n.rx_rate_mbs + n.tx_rate_mbs, 0);
    netSeries.value = [...netSeries.value, total].slice(-40);

    const curCpu = next.thermal.cpu_temp;
    const curGpu = next.thermal.gpu_temp;
    if (curCpu > 0 || curGpu > 0) {
      tempSeries.value = [...tempSeries.value, { cpu: curCpu, gpu: curGpu }].slice(-40);
    }

    fanStatus()
      .then((f) => (fans.value = f))
      .catch(() => (fans.value = []));
    topProcesses(12)
      .then((p) => (procs.value = p))
      .catch(() => (procs.value = []));
  } catch (e) {
    error.value = String(e);
  } finally {
    refreshing.value = false;
  }
}

let pollTimer: ReturnType<typeof setInterval> | null = null;

onMounted(() => {
  refresh();
  pollTimer = setInterval(refresh, 5000);

  const handleClickOutside = (e: MouseEvent) => {
    const target = e.target as Node;
    if (
      showNetPopover.value &&
      popoverRef.value &&
      !popoverRef.value.contains(target) &&
      chipRef.value &&
      !chipRef.value.contains(target)
    ) {
      showNetPopover.value = false;
    }

    if (menuPid.value !== null && menuRef.value && !menuRef.value.contains(target)) {
      menuPid.value = null;
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      menuPid.value = null;
      showNetPopover.value = false;
    }
  };

  document.addEventListener("mousedown", handleClickOutside);
  document.addEventListener("keydown", handleKeyDown);

  onUnmounted(() => {
    if (pollTimer) clearInterval(pollTimer);
    document.removeEventListener("mousedown", handleClickOutside);
    document.removeEventListener("keydown", handleKeyDown);
  });
});

const primaryDisk = computed(() => {
  if (!snap.value) return undefined;
  return snap.value.disks.find((d) => d.mount === "/") ?? snap.value.disks[0];
});

const cpuTemp = computed(() => snap.value?.thermal.cpu_temp ?? 0);
const gpuTemp = computed(() => snap.value?.thermal.gpu_temp ?? 0);

const peakCpu = computed(() => {
  return tempSeries.value.reduce((max, p) => Math.max(max, p.cpu), cpuTemp.value);
});

const peakGpu = computed(() => {
  return tempSeries.value.reduce((max, p) => Math.max(max, p.gpu), gpuTemp.value);
});

const fan = computed(() => fans.value[0]);
const fanPercent = computed(() => {
  const f = fan.value;
  return f && f.max_rpm > 0 ? Math.round((f.rpm / f.max_rpm) * 100) : null;
});

const memPressure = computed(() => {
  if (!snap.value) return { key: "pressureNormal" as PressureKey, cls: "ok" };
  return memoryPressure(snap.value.memory);
});

const midRpm = computed(() => {
  const f = fan.value;
  return f && f.max_rpm > 0
    ? Math.min(Math.max(2500, Math.round(f.min_rpm)), Math.round(f.max_rpm))
    : 2500;
});

const primaryNet = computed(() => {
  if (!snap.value) return undefined;
  return (
    snap.value.network.find((n) => /^(en|wl)/.test(n.name) && (n.rx_rate_mbs > 0 || n.tx_rate_mbs > 0)) ||
    snap.value.network.find((n) => /^(en|wl)/.test(n.name)) ||
    snap.value.network.find((n) => !/^(lo|stf|gif|utun|anpi|awdl|llw|bridge)/.test(n.name)) ||
    snap.value.network[0]
  );
});

const rxRate = computed(() => primaryNet.value?.rx_rate_mbs ?? 0);
const txRate = computed(() => primaryNet.value?.tx_rate_mbs ?? 0);
const netName = computed(() => primaryNet.value?.name ?? "");
const isWifi = computed(() => {
  return /^(en|wl)/.test(netName.value) && !/^(utun|lo|stf|gif|bridge|anpi|awdl)/.test(netName.value);
});

const free = computed(() => {
  return primaryDisk.value ? splitBytes(primaryDisk.value.total - primaryDisk.value.used) : null;
});

const sortedProcs = computed(() => {
  return [...procs.value].sort((a, b) => {
    const aPin = pinnedPids.value.includes(a.pid);
    const bPin = pinnedPids.value.includes(b.pid);
    if (aPin && !bPin) return -1;
    if (!aPin && bPin) return 1;
    return b.cpu_percent - a.cpu_percent;
  });
});
</script>
