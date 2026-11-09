<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { Plus, Server, SquareTerminal } from "lucide-vue-next";
import Sidebar from "@/components/sidebar/Sidebar.vue";
import TabBar from "@/components/tabs/TabBar.vue";
import TerminalView from "@/components/terminal/TerminalView.vue";
import InfoPanel from "@/components/info/InfoPanel.vue";
import StatusBar from "@/components/common/StatusBar.vue";
import { useTabsStore } from "@/stores/tabs";
import { useUiStore } from "@/stores/ui";
import { useConnectionsStore } from "@/stores/connections";
import { useConnectionActions } from "@/modules/connections/actions";

const tabs = useTabsStore();
const ui = useUiStore();
const connections = useConnectionsStore();
const actions = useConnectionActions();

const MIN_SIDEBAR = 200;
const MAX_SIDEBAR = 480;
const dragging = ref(false);

function startDrag(e: PointerEvent) {
  dragging.value = true;
  const startX = e.clientX;
  const startW = ui.sidebarWidth;
  const move = (ev: PointerEvent) => {
    ui.sidebarWidth = Math.min(MAX_SIDEBAR, Math.max(MIN_SIDEBAR, startW + ev.clientX - startX));
  };
  const up = () => {
    dragging.value = false;
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}

/**
 * Global shortcuts. Ctrl+Shift variants work everywhere; plain Ctrl+N / Ctrl+W are
 * only used outside the terminal (inside it they belong to the shell).
 */
function onKey(e: KeyboardEvent) {
  const mod = e.ctrlKey || e.metaKey;
  if (!mod) return;
  const key = e.key.toLowerCase();
  const inTerminal = !!(e.target as HTMLElement | null)?.closest?.(".xterm");
  if (key === "tab") {
    e.preventDefault();
    tabs.cycle(e.shiftKey ? -1 : 1);
  } else if (key === "w" && tabs.activeId && (e.shiftKey || !inTerminal)) {
    e.preventDefault();
    void tabs.close(tabs.activeId);
  } else if (key === "n" && (e.shiftKey || !inTerminal)) {
    e.preventDefault();
    actions.newConnection();
  } else if (key === "i" && e.altKey) {
    e.preventDefault();
    ui.infoPanelOpen = !ui.infoPanelOpen;
  }
}

onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));

const recent = () =>
  [...connections.connections]
    .filter((c) => c.last_connected_at)
    .sort((a, b) => (b.last_connected_at ?? "").localeCompare(a.last_connected_at ?? ""))
    .slice(0, 5);
</script>

<template>
  <div class="flex h-full flex-col" :class="dragging ? 'cursor-col-resize select-none' : ''">
    <div class="flex min-h-0 flex-1">
      <!-- sidebar -->
      <div class="relative shrink-0 border-r border-line" :style="{ width: ui.sidebarWidth + 'px' }">
        <Sidebar />
        <div
          class="absolute top-0 -right-1 z-10 h-full w-2 cursor-col-resize hover:bg-accent/30"
          :class="dragging ? 'bg-accent/40' : ''"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize sidebar"
          @pointerdown.prevent="startDrag"
        />
      </div>

      <!-- main workspace -->
      <main class="flex min-w-0 flex-1 flex-col bg-surface">
        <TabBar v-if="tabs.tabs.length" />
        <div class="relative min-h-0 flex-1">
          <!-- Every terminal stays mounted (v-show) so its buffer survives tab switches. -->
          <div v-for="t in tabs.tabs" v-show="t.id === tabs.activeId" :key="t.id" class="absolute inset-0" role="tabpanel">
            <TerminalView :tab="t" :active="t.id === tabs.activeId" />
          </div>

          <!-- welcome -->
          <div v-if="!tabs.tabs.length" class="flex h-full flex-col items-center justify-center px-8 text-center">
            <div class="flex size-14 items-center justify-center rounded-2xl bg-accent-soft text-accent">
              <SquareTerminal :size="28" />
            </div>
            <h1 class="mt-4 text-lg font-semibold">Welcome to DevNest</h1>
            <p class="mt-1 max-w-sm text-fg-muted">
              Double-click a connection in the sidebar to open an SSH terminal, or create a new one.
            </p>
            <div class="mt-5 flex gap-2">
              <button class="btn btn-primary" @click="actions.newConnection()"><Plus :size="14" /> New connection</button>
            </div>
            <div v-if="recent().length" class="mt-8 w-full max-w-sm text-left">
              <p class="mb-2 text-[11px] font-semibold tracking-wider text-fg-muted uppercase">Recent</p>
              <button
                v-for="c in recent()"
                :key="c.id"
                class="flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left hover:bg-hover"
                @click="actions.openTerminal(c)"
              >
                <Server :size="14" class="text-fg-subtle" />
                <span class="truncate">{{ c.name }}</span>
                <span class="ml-auto truncate text-xs text-fg-subtle">{{ c.username }}@{{ c.host }}</span>
              </button>
            </div>
            <p class="mt-8 text-xs text-fg-subtle">
              <kbd class="rounded border border-line px-1">Ctrl</kbd>+<kbd class="rounded border border-line px-1">Shift</kbd>+<kbd class="rounded border border-line px-1">N</kbd> new connection ·
              <kbd class="rounded border border-line px-1">Ctrl</kbd>+<kbd class="rounded border border-line px-1">Tab</kbd> switch tab ·
              <kbd class="rounded border border-line px-1">Ctrl</kbd>+<kbd class="rounded border border-line px-1">Shift</kbd>+<kbd class="rounded border border-line px-1">W</kbd> close tab
            </p>
          </div>
        </div>
      </main>

      <!-- info panel -->
      <div v-if="ui.infoPanelOpen" class="w-[260px] shrink-0">
        <InfoPanel />
      </div>
    </div>
    <StatusBar />
  </div>
</template>
