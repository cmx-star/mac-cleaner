/** 界面用的小图标。用内联 SVG，不引入图标库。Vue 3 组件实现。 */
import { defineComponent, h } from "vue";

export interface StatusIconProps {
  size?: number;
  color?: string;
  class?: string;
}

/** 刷新：扫描中整体旋转。 */
export const RefreshIcon = defineComponent({
  name: "RefreshIcon",
  props: {
    spinning: { type: Boolean, default: false },
  },
  setup(props) {
    return () =>
      h(
        "svg",
        {
          class: props.spinning ? "icon-spin" : undefined,
          width: "14",
          height: "14",
          viewBox: "0 0 16 16",
          fill: "none",
          stroke: "currentColor",
          strokeWidth: "1.6",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          "aria-hidden": "true",
        },
        [
          h("path", { d: "M13.5 8a5.5 5.5 0 1 1-1.6-3.9" }),
          h("path", { d: "M13.5 1.8v2.6h-2.6" }),
        ]
      );
  },
});

/** 搜索。 */
export const SearchIcon = defineComponent({
  name: "SearchIcon",
  setup() {
    return () =>
      h(
        "svg",
        {
          width: "14",
          height: "14",
          viewBox: "0 0 16 16",
          fill: "none",
          stroke: "currentColor",
          strokeWidth: "1.6",
          strokeLinecap: "round",
          "aria-hidden": "true",
        },
        [
          h("circle", { cx: "7", cy: "7", r: "4.5" }),
          h("path", { d: "M10.5 10.5 14 14" }),
        ]
      );
  },
});

/** 下拉展开箭头。 */
export const ChevronDownIcon = defineComponent({
  name: "ChevronDownIcon",
  props: {
    open: { type: Boolean, default: false },
  },
  setup(props) {
    return () =>
      h(
        "svg",
        {
          style: {
            transform: props.open ? "rotate(180deg)" : "rotate(0deg)",
            transition: "transform 0.2s ease",
          },
          width: "14",
          height: "14",
          viewBox: "0 0 16 16",
          fill: "none",
          stroke: "currentColor",
          strokeWidth: "1.8",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          "aria-hidden": "true",
        },
        [h("path", { d: "M4 6l4 4 4-4" })]
      );
  },
});

/** 勾选对号。 */
export const CheckIcon = defineComponent({
  name: "CheckIcon",
  setup() {
    return () =>
      h(
        "svg",
        {
          width: "11",
          height: "11",
          viewBox: "0 0 16 16",
          fill: "none",
          stroke: "currentColor",
          strokeWidth: "2.4",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          "aria-hidden": "true",
        },
        [h("path", { d: "M3.5 8.5l3 3 6-7" })]
      );
  },
});

/** 半选/减号。 */
export const MinusIcon = defineComponent({
  name: "MinusIcon",
  setup() {
    return () =>
      h(
        "svg",
        {
          width: "11",
          height: "11",
          viewBox: "0 0 16 16",
          fill: "none",
          stroke: "currentColor",
          strokeWidth: "2.4",
          strokeLinecap: "round",
          "aria-hidden": "true",
        },
        [h("path", { d: "M3.5 8h9" })]
      );
  },
});

/** 健康评分：心形图标 */
export const HealthIcon = defineComponent({
  name: "HealthIcon",
  props: {
    size: { type: Number, default: 14 },
    color: { type: String, default: "var(--green)" },
    class: { type: String, default: undefined },
  },
  setup(props) {
    return () =>
      h(
        "svg",
        {
          width: props.size,
          height: props.size,
          viewBox: "0 0 24 24",
          fill: "none",
          stroke: props.color,
          strokeWidth: "2",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          class: props.class,
          "aria-hidden": "true",
        },
        [
          h("path", {
            d: "M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z",
          }),
        ]
      );
  },
});

/** CPU 处理器图标 */
export const CpuIcon = defineComponent({
  name: "CpuIcon",
  props: {
    size: { type: Number, default: 14 },
    color: { type: String, default: "var(--accent)" },
    class: { type: String, default: undefined },
  },
  setup(props) {
    return () =>
      h(
        "svg",
        {
          width: props.size,
          height: props.size,
          viewBox: "0 0 24 24",
          fill: "none",
          stroke: props.color,
          strokeWidth: "2",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          class: props.class,
          "aria-hidden": "true",
        },
        [
          h("rect", { x: "4", y: "4", width: "16", height: "16", rx: "2" }),
          h("rect", { x: "9", y: "9", width: "6", height: "6" }),
          h("path", { d: "M15 2v2" }),
          h("path", { d: "M15 20v2" }),
          h("path", { d: "M2 15h2" }),
          h("path", { d: "M2 9h2" }),
          h("path", { d: "M20 15h2" }),
          h("path", { d: "M20 9h2" }),
          h("path", { d: "M9 2v2" }),
          h("path", { d: "M9 20v2" }),
        ]
      );
  },
});

/** GPU 图形处理器图标 */
export const GpuIcon = defineComponent({
  name: "GpuIcon",
  props: {
    size: { type: Number, default: 14 },
    color: { type: String, default: "#a78bfa" },
    class: { type: String, default: undefined },
  },
  setup(props) {
    return () =>
      h(
        "svg",
        {
          width: props.size,
          height: props.size,
          viewBox: "0 0 24 24",
          fill: "none",
          stroke: props.color,
          strokeWidth: "2",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          class: props.class,
          "aria-hidden": "true",
        },
        [
          h("path", { d: "M2 17h18a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2H2" }),
          h("path", { d: "M2 21V3" }),
          h("path", { d: "M18 5v12" }),
          h("path", { d: "M6 9v2" }),
          h("path", { d: "M6 13v2" }),
          h("path", { d: "M10 9v2" }),
          h("path", { d: "M10 13v2" }),
          h("path", { d: "M14 9v2" }),
          h("path", { d: "M14 13v2" }),
        ]
      );
  },
});

/** 内存 RAM 图标 */
export const MemoryIcon = defineComponent({
  name: "MemoryIcon",
  props: {
    size: { type: Number, default: 14 },
    color: { type: String, default: "#38bdf8" },
    class: { type: String, default: undefined },
  },
  setup(props) {
    return () =>
      h(
        "svg",
        {
          width: props.size,
          height: props.size,
          viewBox: "0 0 24 24",
          fill: "none",
          stroke: props.color,
          strokeWidth: "2",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          class: props.class,
          "aria-hidden": "true",
        },
        [
          h("path", { d: "M12 12v-2" }),
          h("path", { d: "M12 18v-2" }),
          h("path", { d: "M16 12v-2" }),
          h("path", { d: "M16 18v-2" }),
          h("path", { d: "M2 11h1.5" }),
          h("path", { d: "M20 18v-2" }),
          h("path", { d: "M20.5 11H22" }),
          h("path", { d: "M4 18v-2" }),
          h("path", { d: "M8 12v-2" }),
          h("path", { d: "M8 18v-2" }),
          h("rect", { x: "2", y: "6", width: "20", height: "10", rx: "2" }),
        ]
      );
  },
});

/** 温度计图标 */
export const ThermalIcon = defineComponent({
  name: "ThermalIcon",
  props: {
    size: { type: Number, default: 14 },
    color: { type: String, default: "var(--red)" },
    class: { type: String, default: undefined },
  },
  setup(props) {
    return () =>
      h(
        "svg",
        {
          width: props.size,
          height: props.size,
          viewBox: "0 0 24 24",
          fill: "none",
          stroke: props.color,
          strokeWidth: "2",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          class: props.class,
          "aria-hidden": "true",
        },
        [
          h("path", {
            d: "M14 14.76V3.5a2.5 2.5 0 0 0-5 0v11.26a4.5 4.5 0 1 0 5 0z",
          }),
        ]
      );
  },
});

/** 可用空间 / 硬盘图标 */
export const DiskIcon = defineComponent({
  name: "DiskIcon",
  props: {
    size: { type: Number, default: 14 },
    color: { type: String, default: "#2dd4bf" },
    class: { type: String, default: undefined },
  },
  setup(props) {
    return () =>
      h(
        "svg",
        {
          width: props.size,
          height: props.size,
          viewBox: "0 0 24 24",
          fill: "none",
          stroke: props.color,
          strokeWidth: "2",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          class: props.class,
          "aria-hidden": "true",
        },
        [
          h("line", { x1: "22", y1: "12", x2: "2", y2: "12" }),
          h("path", {
            d: "M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z",
          }),
          h("line", { x1: "6", y1: "16", x2: "6.01", y2: "16" }),
          h("line", { x1: "10", y1: "16", x2: "10.01", y2: "16" }),
        ]
      );
  },
});

/** Wi-Fi 图标 */
export const WifiIcon = defineComponent({
  name: "WifiIcon",
  props: {
    size: { type: Number, default: 14 },
    color: { type: String, default: "var(--accent)" },
    class: { type: String, default: undefined },
  },
  setup(props) {
    return () =>
      h(
        "svg",
        {
          width: props.size,
          height: props.size,
          viewBox: "0 0 24 24",
          fill: "none",
          stroke: props.color,
          strokeWidth: "2",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          class: props.class,
          "aria-hidden": "true",
        },
        [
          h("path", { d: "M12 20h.01" }),
          h("path", { d: "M2 8.82a15 15 0 0 1 20 0" }),
          h("path", { d: "M5 12.859a10 10 0 0 1 14 0" }),
          h("path", { d: "M8.5 16.429a5 5 0 0 1 7 0" }),
        ]
      );
  },
});

/** 网络图标（根据当前网络接口自动渲染 Wi-Fi 或以太网有线图标） */
export const NetworkIcon = defineComponent({
  name: "NetworkIcon",
  props: {
    isWifi: { type: Boolean, default: true },
    size: { type: Number, default: 14 },
    color: { type: String, default: "var(--accent)" },
    class: { type: String, default: undefined },
  },
  setup(props) {
    return () => {
      if (props.isWifi) {
        return h(WifiIcon, {
          size: props.size,
          color: props.color,
          class: props.class,
        });
      }
      return h(
        "svg",
        {
          width: props.size,
          height: props.size,
          viewBox: "0 0 24 24",
          fill: "none",
          stroke: props.color,
          strokeWidth: "2",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          class: props.class,
          "aria-hidden": "true",
        },
        [
          h("rect", { x: "16", y: "16", width: "6", height: "6", rx: "1" }),
          h("rect", { x: "2", y: "16", width: "6", height: "6", rx: "1" }),
          h("rect", { x: "9", y: "2", width: "6", height: "6", rx: "1" }),
          h("path", { d: "M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3" }),
          h("path", { d: "M12 12V8" }),
        ]
      );
    };
  },
});

/** 风扇图标 */
export const FanIcon = defineComponent({
  name: "FanIcon",
  props: {
    size: { type: Number, default: 14 },
    color: { type: String, default: "var(--yellow)" },
    class: { type: String, default: undefined },
  },
  setup(props) {
    return () =>
      h(
        "svg",
        {
          width: props.size,
          height: props.size,
          viewBox: "0 0 24 24",
          fill: "none",
          stroke: props.color,
          strokeWidth: "2",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          class: props.class,
          "aria-hidden": "true",
        },
        [
          h("path", {
            d: "M12 12c-2.8 0-5-2.2-5-5 0-2.2 1.8-4 4-4 3.9 0 6 4.1 4.5 7.5L12 12z",
          }),
          h("path", {
            d: "M12 12c0-2.8 2.2-5 5-5 2.2 0 4 1.8 4 4 0 3.9-4.1 6-7.5 4.5L12 12z",
          }),
          h("path", {
            d: "M12 12c2.8 0 5 2.2 5 5 0 2.2-1.8 4-4 4-3.9 0-6-4.1-4.5-7.5L12 12z",
          }),
          h("path", {
            d: "M12 12c0 2.8-2.2 5-5 5-2.2 0-4-1.8-4-4 0-3.9 4.1-6 7.5-4.5L12 12z",
          }),
          h("circle", { cx: "12", cy: "12", r: "2", fill: "currentColor" }),
        ]
      );
  },
});
