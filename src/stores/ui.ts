import { defineStore } from "pinia";
import { ref, shallowRef, watch } from "vue";
import type { Connection } from "@/modules/connections/types";

export type Theme = "light" | "dark" | "system";

function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

function save(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* storage unavailable – preferences just won't persist */
  }
}

const media = typeof window !== "undefined" ? window.matchMedia("(prefers-color-scheme: dark)") : null;

/** Per-machine UI preferences (theme, panel sizes). */
export const useUiStore = defineStore("ui", () => {
  const theme = ref<Theme>(readTheme());
  const isDark = ref(false);
  const sidebarWidth = ref<number>(load("devnest.sidebarWidth", 260));
  const infoPanelOpen = ref<boolean>(load("devnest.infoPanel", true));
  /** Connection editor dialog: `undefined` = closed, `null` = new connection. */
  const editor = shallowRef<{ connection: Connection | null; groupId?: string | null } | undefined>(undefined);

  function openEditor(connection: Connection | null = null, groupId: string | null = null) {
    editor.value = { connection, groupId };
  }
  function closeEditor() {
    editor.value = undefined;
  }

  function readTheme(): Theme {
    try {
      const t = localStorage.getItem("devnest.theme");
      return t === "light" || t === "dark" || t === "system" ? t : "system";
    } catch {
      return "system";
    }
  }

  function applyTheme() {
    isDark.value = theme.value === "dark" || (theme.value === "system" && !!media?.matches);
    document.documentElement.classList.toggle("dark", isDark.value);
  }

  function cycleTheme() {
    const order: Theme[] = ["system", "light", "dark"];
    theme.value = order[(order.indexOf(theme.value) + 1) % order.length];
  }

  watch(
    theme,
    (t) => {
      try {
        localStorage.setItem("devnest.theme", t);
      } catch {
        /* ignore */
      }
      applyTheme();
    },
    { immediate: true },
  );
  media?.addEventListener("change", applyTheme);

  watch(sidebarWidth, (w) => save("devnest.sidebarWidth", w));
  watch(infoPanelOpen, (v) => save("devnest.infoPanel", v));

  return { theme, isDark, sidebarWidth, infoPanelOpen, cycleTheme, editor, openEditor, closeEditor };
});
