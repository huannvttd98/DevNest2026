<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Terminal, type ITheme } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { SearchAddon } from "@xterm/addon-search";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ChevronDown, ChevronUp, Files, LoaderCircle, RotateCw, Search, Unplug, X } from "lucide-vue-next";
import { createTerminalChannel, sshApi } from "@/modules/ssh/api";
import { openSftpFileBrowser } from "@/modules/ssh/file-browser";
import { CancelledError, runWithPrompts } from "@/modules/ssh/prompts";
import { useConnectionsStore } from "@/stores/connections";
import { useTabsStore, type TerminalTab } from "@/stores/tabs";
import { useToastStore } from "@/stores/toasts";
import { useUiStore } from "@/stores/ui";
import { toAppError } from "@/lib/ipc";

const props = defineProps<{ tab: TerminalTab; active: boolean }>();

const tabs = useTabsStore();
const ui = useUiStore();
const connections = useConnectionsStore();
const toasts = useToastStore();

const host = ref<HTMLDivElement | null>(null);
const searchOpen = ref(false);
const searchText = ref("");
const searchInput = ref<HTMLInputElement | null>(null);
const openingFileBrowser = ref(false);

let term: Terminal | null = null;
let fit: FitAddon | null = null;
let search: SearchAddon | null = null;
let resizeObserver: ResizeObserver | null = null;
let fitTimer: number | undefined;
let resizeTimer: number | undefined;
let lastSize = { cols: 0, rows: 0 };
/** Incremented per connect attempt so late events from an old shell are ignored. */
let generation = 0;

const DARK: ITheme = {
  background: "#14171a",
  foreground: "#d8dde5",
  cursor: "#5b8dff",
  cursorAccent: "#14171a",
  selectionBackground: "#2d4a80",
  black: "#1f2328",
  red: "#f06363",
  green: "#4cc27f",
  yellow: "#e0a83c",
  blue: "#5b8dff",
  magenta: "#c678dd",
  cyan: "#43b9c6",
  white: "#d8dde5",
  brightBlack: "#6f7785",
  brightRed: "#ff7b7b",
  brightGreen: "#6bd89a",
  brightYellow: "#f0c060",
  brightBlue: "#7fa6ff",
  brightMagenta: "#d896ea",
  brightCyan: "#64d2de",
  brightWhite: "#ffffff",
};
const LIGHT: ITheme = {
  background: "#ffffff",
  foreground: "#1d2129",
  cursor: "#2f6fed",
  cursorAccent: "#ffffff",
  selectionBackground: "#c9dafc",
  black: "#1d2129",
  red: "#c62f2f",
  green: "#22863a",
  yellow: "#9a6700",
  blue: "#2f6fed",
  magenta: "#8250df",
  cyan: "#1b7c83",
  white: "#d0d4db",
  brightBlack: "#5b6270",
  brightRed: "#d43c3c",
  brightGreen: "#2f9e5b",
  brightYellow: "#b7791f",
  brightBlue: "#4b84f5",
  brightMagenta: "#a371f7",
  brightCyan: "#3192aa",
  brightWhite: "#ffffff",
};

function notice(text: string, color = "90") {
  term?.write(`\r\n\x1b[${color}m${text}\x1b[0m\r\n`);
}

function doFit() {
  if (!term || !fit || !host.value || host.value.offsetWidth === 0 || host.value.offsetHeight === 0) return;
  try {
    fit.fit();
  } catch {
    return;
  }
  const { cols, rows } = term;
  if ((cols !== lastSize.cols || rows !== lastSize.rows) && props.tab.terminalId && props.tab.state === "connected") {
    lastSize = { cols, rows };
    const tid = props.tab.terminalId;
    clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(() => void sshApi.resize(tid, cols, rows).catch(() => {}), 80);
  }
}

function scheduleFit() {
  clearTimeout(fitTimer);
  fitTimer = window.setTimeout(doFit, 30);
}

async function connect() {
  if (!term) return;
  const conn = connections.byId.get(props.tab.connectionId);
  if (!conn) {
    tabs.patch(props.tab.id, { state: "error", message: "This connection no longer exists." });
    return;
  }
  const gen = ++generation;
  tabs.patch(props.tab.id, { state: "connecting", message: undefined, terminalId: null });
  doFit();
  let saved = false;

  try {
    const result = await runWithPrompts(
      { connectionName: conn.name, username: conn.username, host: conn.host, allowSave: true },
      (overrides, save) => {
        saved = save;
        const channel = createTerminalChannel({
          onData: (bytes) => {
            if (gen === generation) term?.write(bytes);
          },
          onEvent: (ev) => {
            if (gen !== generation) return;
            if (ev.type === "exit") {
              notice(ev.code !== null ? `[process exited with code ${ev.code}]` : `[process terminated: ${ev.signal}]`);
            } else if (ev.type === "closed") {
              tabs.patch(props.tab.id, { state: "closed", terminalId: null, message: ev.reason ?? "Session closed" });
              notice(`[${ev.reason ?? "session closed"}] — press Enter to reconnect`, "33");
            }
          },
        });
        return sshApi.openTerminal({
          connectionId: conn.id,
          cols: term?.cols ?? 80,
          rows: term?.rows ?? 24,
          overrides,
          saveOverrides: save,
          channel,
        });
      },
    );
    if (gen !== generation) return;
    // Do not overwrite a "closed" event that may already have arrived.
    if (props.tab.state === "connecting") {
      tabs.patch(props.tab.id, { state: "connected", terminalId: result.terminal_id });
    }
    lastSize = { cols: term.cols, rows: term.rows };
    if (saved) void connections.refresh();
    if (props.active) term.focus();
  } catch (e) {
    if (gen !== generation) return;
    if (e instanceof CancelledError) {
      tabs.patch(props.tab.id, { state: "closed", message: "Connection cancelled" });
      notice("[connection cancelled] — press Enter to retry", "33");
      return;
    }
    const err = toAppError(e);
    tabs.patch(props.tab.id, { state: "error", message: err.message });
    notice(`[${err.message}]`, "31");
  }
}

function send(data: string, binary = false) {
  const t = props.tab;
  if (t.state === "connected" && t.terminalId) {
    sshApi.write(t.terminalId, data, binary).catch(() => {
      /* the reader loop reports the close */
    });
  } else if ((t.state === "closed" || t.state === "error") && data === "\r") {
    void connect();
  }
}

function copySelection(): boolean {
  const sel = term?.getSelection();
  if (!sel) return false;
  void navigator.clipboard?.writeText(sel).catch(() => {});
  return true;
}

function keyHandler(e: KeyboardEvent): boolean {
  if (e.type !== "keydown") return true;
  const mod = e.ctrlKey || e.metaKey;
  const key = e.key.toLowerCase();
  // Copy: Ctrl+Shift+C, or Ctrl+C when text is selected (otherwise Ctrl+C = SIGINT).
  if (mod && key === "c" && (e.shiftKey || term?.hasSelection())) {
    copySelection();
    term?.clearSelection();
    e.preventDefault();
    return false;
  }
  // Paste: let the browser fire a native paste event that xterm handles.
  if (mod && key === "v") return false;
  if (mod && e.shiftKey && key === "f") {
    e.preventDefault();
    openSearch();
    return false;
  }
  // App-level shortcuts are handled by the layout (see AppLayout.onKey).
  if (mod && key === "tab") return false;
  if (mod && e.shiftKey && (key === "w" || key === "n")) return false;
  if (mod && e.altKey && key === "i") return false;
  return true;
}

function openSearch() {
  searchOpen.value = true;
  void nextTick(() => searchInput.value?.select());
}

function findNext(prev = false) {
  if (!search || !searchText.value) return;
  const opts = { caseSensitive: false, decorations: { matchOverviewRuler: "#e0a83c", activeMatchColorOverviewRuler: "#5b8dff" } };
  if (prev) search.findPrevious(searchText.value, opts);
  else search.findNext(searchText.value, opts);
}

function closeSearch() {
  searchOpen.value = false;
  search?.clearDecorations();
  term?.focus();
}

async function openFileBrowser() {
  const conn = connections.byId.get(props.tab.connectionId);
  if (!conn || props.tab.state !== "connected" || openingFileBrowser.value) return;
  openingFileBrowser.value = true;
  try {
    await openSftpFileBrowser(conn);
  } catch (error) {
    toasts.error("Cannot open server files", toAppError(error).message);
  } finally {
    openingFileBrowser.value = false;
  }
}

onMounted(() => {
  term = new Terminal({
    fontFamily: getComputedStyle(document.documentElement).getPropertyValue("--font-mono") || "monospace",
    fontSize: 13,
    lineHeight: 1.15,
    cursorBlink: true,
    scrollback: 10000,
    allowProposedApi: true,
    theme: ui.isDark ? DARK : LIGHT,
  });
  fit = new FitAddon();
  search = new SearchAddon();
  term.loadAddon(fit);
  term.loadAddon(search);
  term.loadAddon(
    new WebLinksAddon((ev, uri) => {
      // Open links with Ctrl/Cmd+click only, in the system browser.
      if (ev.ctrlKey || ev.metaKey) void openUrl(uri).catch(() => {});
    }),
  );
  term.attachCustomKeyEventHandler(keyHandler);
  term.onData((d) => send(d));
  term.onBinary((d) => send(d, true));
  term.open(host.value!);
  host.value!.addEventListener("contextmenu", (e) => {
    if (copySelection()) e.preventDefault();
  });

  resizeObserver = new ResizeObserver(scheduleFit);
  resizeObserver.observe(host.value!);
  doFit();
  void connect();
});

watch(
  () => props.active,
  (a) => {
    if (a) {
      void nextTick(() => {
        doFit();
        term?.focus();
      });
    }
  },
);

watch(
  () => ui.isDark,
  (dark) => {
    if (term) term.options.theme = dark ? DARK : LIGHT;
  },
);

onBeforeUnmount(() => {
  generation++;
  clearTimeout(fitTimer);
  clearTimeout(resizeTimer);
  resizeObserver?.disconnect();
  term?.dispose();
  term = null;
});

defineExpose({ reconnect: connect, focus: () => term?.focus() });
</script>

<template>
  <div class="relative flex h-full flex-col" :style="{ background: 'var(--terminal-bg)' }">
    <!-- search -->
    <div
      v-if="searchOpen"
      class="absolute top-10 right-4 z-10 flex items-center gap-1 rounded-md border border-line bg-surface p-1 shadow-lg"
    >
      <Search :size="13" class="ml-1 text-fg-subtle" />
      <input
        ref="searchInput"
        v-model="searchText"
        class="h-6 w-48 bg-transparent px-1 text-xs outline-none"
        placeholder="Find in terminal"
        aria-label="Find in terminal"
        @keydown.enter.exact.prevent="findNext()"
        @keydown.shift.enter.prevent="findNext(true)"
        @keydown.esc.prevent="closeSearch"
      />
      <button class="icon-btn size-6" aria-label="Previous match" @click="findNext(true)"><ChevronUp :size="14" /></button>
      <button class="icon-btn size-6" aria-label="Next match" @click="findNext()"><ChevronDown :size="14" /></button>
      <button class="icon-btn size-6" aria-label="Close search" @click="closeSearch"><X :size="14" /></button>
    </div>

    <div class="flex h-8 shrink-0 items-center justify-end border-b border-line/60 px-2">
      <button
        class="btn h-6 text-xs"
        :disabled="tab.state !== 'connected' || openingFileBrowser"
        :title="
          tab.state === 'connected' ? 'Browse and download files from this server' : 'Connect to browse server files'
        "
        @click="openFileBrowser"
      >
        <LoaderCircle v-if="openingFileBrowser" :size="13" class="animate-spin" />
        <Files v-else :size="13" />
        {{ openingFileBrowser ? "Opening…" : "Server files" }}
      </button>
    </div>

    <div class="relative min-h-0 flex-1 px-2 pt-1.5">
      <div ref="host" class="absolute inset-0 left-2 top-1.5" />
    </div>

    <!-- connecting overlay -->
    <div
      v-if="tab.state === 'connecting'"
      class="pointer-events-none absolute inset-x-0 top-0 flex justify-center pt-6"
      role="status"
    >
      <div class="flex items-center gap-2 rounded-full border border-line bg-surface px-3 py-1.5 text-xs shadow">
        <LoaderCircle :size="14" class="animate-spin text-accent" /> Connecting…
      </div>
    </div>

    <!-- disconnected bar -->
    <div
      v-if="tab.state === 'closed' || tab.state === 'error'"
      class="flex shrink-0 items-center gap-3 border-t px-3 py-2 text-xs"
      :class="tab.state === 'error' ? 'border-danger/30 bg-danger-soft text-danger' : 'border-line bg-warn-soft text-fg'"
      role="alert"
    >
      <Unplug :size="14" class="shrink-0" />
      <span class="selectable min-w-0 flex-1 truncate">{{ tab.message ?? "Disconnected" }}</span>
      <button class="btn h-7 text-xs" @click="connect"><RotateCw :size="13" /> Reconnect</button>
    </div>
  </div>
</template>

<style scoped>
:deep(.xterm) {
  height: 100%;
}
:deep(.xterm-viewport) {
  background-color: transparent !important;
}
</style>
