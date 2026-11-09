import { defineStore } from "pinia";
import { ref } from "vue";

export type ToastKind = "info" | "success" | "error" | "warning";

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  message?: string;
}

let nextId = 1;

export const useToastStore = defineStore("toasts", () => {
  const items = ref<Toast[]>([]);

  function push(kind: ToastKind, title: string, message?: string, timeout = kind === "error" ? 7000 : 3500) {
    const id = nextId++;
    items.value.push({ id, kind, title, message });
    if (timeout > 0) setTimeout(() => dismiss(id), timeout);
  }

  function dismiss(id: number) {
    items.value = items.value.filter((t) => t.id !== id);
  }

  return {
    items,
    dismiss,
    info: (t: string, m?: string) => push("info", t, m),
    success: (t: string, m?: string) => push("success", t, m),
    warning: (t: string, m?: string) => push("warning", t, m),
    error: (t: string, m?: string) => push("error", t, m),
  };
});
