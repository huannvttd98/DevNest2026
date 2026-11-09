import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { sshApi } from "@/modules/ssh/api";
import type { Connection } from "@/modules/connections/types";

export type TerminalState = "connecting" | "connected" | "closed" | "error";

export interface TerminalTab {
  id: string;
  kind: "terminal";
  connectionId: string;
  title: string;
  customTitle: boolean;
  /** Backend terminal id of the current shell (changes on reconnect). */
  terminalId: string | null;
  state: TerminalState;
  message?: string;
}

export type Tab = TerminalTab;

let seq = 1;

export const useTabsStore = defineStore("tabs", () => {
  const tabs = ref<Tab[]>([]);
  const activeId = ref<string | null>(null);

  const active = computed(() => tabs.value.find((t) => t.id === activeId.value) ?? null);

  function get(id: string) {
    return tabs.value.find((t) => t.id === id);
  }

  function uniqueTitle(base: string) {
    const used = new Set(tabs.value.map((t) => t.title));
    if (!used.has(base)) return base;
    let n = 2;
    while (used.has(`${base} (${n})`)) n++;
    return `${base} (${n})`;
  }

  function openTerminal(conn: Connection): TerminalTab {
    const tab: TerminalTab = {
      id: `tab-${seq++}`,
      kind: "terminal",
      connectionId: conn.id,
      title: uniqueTitle(conn.name),
      customTitle: false,
      terminalId: null,
      state: "connecting",
    };
    tabs.value.push(tab);
    activeId.value = tab.id;
    return tab;
  }

  function patch(id: string, p: Partial<TerminalTab>) {
    const t = get(id);
    if (t) Object.assign(t, p);
  }

  function activate(id: string) {
    if (get(id)) activeId.value = id;
  }

  function rename(id: string, title: string) {
    const t = get(id);
    const v = title.trim();
    if (t && v) {
      t.title = v;
      t.customTitle = true;
    }
  }

  async function close(id: string) {
    const i = tabs.value.findIndex((t) => t.id === id);
    if (i < 0) return;
    const [tab] = tabs.value.splice(i, 1);
    if (activeId.value === id) {
      const next = tabs.value[i] ?? tabs.value[i - 1] ?? null;
      activeId.value = next?.id ?? null;
    }
    if (tab.terminalId) {
      try {
        await sshApi.close(tab.terminalId);
      } catch {
        /* already closed */
      }
    }
  }

  function closeForConnection(connectionId: string) {
    for (const t of tabs.value.filter((t) => t.connectionId === connectionId)) void close(t.id);
  }

  /** Keep default titles in sync when a connection is renamed. */
  function syncTitles(conn: Connection) {
    for (const t of tabs.value) {
      if (t.connectionId === conn.id && !t.customTitle) t.title = conn.name;
    }
  }

  function cycle(dir: 1 | -1) {
    if (!tabs.value.length) return;
    const i = tabs.value.findIndex((t) => t.id === activeId.value);
    const n = (i + dir + tabs.value.length) % tabs.value.length;
    activeId.value = tabs.value[n].id;
  }

  return { tabs, activeId, active, get, openTerminal, patch, activate, rename, close, closeForConnection, syncTitles, cycle };
});
