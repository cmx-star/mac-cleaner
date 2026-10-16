import { ref, watch, onUnmounted, computed, type MaybeRefOrGetter, toValue, type ComputedRef } from "vue";

/**
 * 逐个扫过目录名的细节文案：前端按已知目录序列轮播。
 */
export function useRotatingDetail(
  dirsSource: MaybeRefOrGetter<string[]>,
  intervalMs = 700
): ComputedRef<string | null> {
  const index = ref(0);
  let timer: ReturnType<typeof setInterval> | null = null;

  const stop = () => {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
  };

  const start = (count: number) => {
    stop();
    index.value = 0;
    if (count > 0) {
      timer = setInterval(() => {
        index.value = (index.value + 1) % count;
      }, intervalMs);
    }
  };

  watch(
    () => toValue(dirsSource),
    (dirs) => {
      const list = dirs || [];
      start(list.length);
    },
    { immediate: true, deep: true }
  );

  onUnmounted(() => {
    stop();
  });

  return computed(() => {
    const list = toValue(dirsSource) || [];
    return list.length > 0 ? list[index.value % list.length] : null;
  });
}
