<script setup lang="ts">
import { ref } from "vue";
import { FileText, FolderTree, LayoutGrid, Monitor, Moon, Server, Sun } from "lucide-vue-next";
import ConnectionsPanel from "./ConnectionsPanel.vue";
import { useUiStore } from "@/stores/ui";

type View = "connections" | "files" | "docs" | "workspaces";
const ui = useUiStore();
const view = ref<View>("connections");

const items = [
  { id: "connections" as const, icon: Server, label: "SSH Connections" },
  { id: "files" as const, icon: FolderTree, label: "File Explorer", soon: "Phase 3" },
  { id: "docs" as const, icon: FileText, label: "Markdown", soon: "Phase 4" },
  { id: "workspaces" as const, icon: LayoutGrid, label: "Workspaces", soon: "Phase 5" },
];
const themeIcon = { light: Sun, dark: Moon, system: Monitor };
</script>

<template>
  <div class="flex h-full">
    <!-- activity bar -->
    <nav class="flex w-11 shrink-0 flex-col items-center gap-1 border-r border-line bg-surface-2 py-2" aria-label="Views">
      <button
        v-for="it in items"
        :key="it.id"
        class="relative flex size-9 items-center justify-center rounded-lg transition-colors"
        :class="view === it.id ? 'bg-accent-soft text-accent' : 'text-fg-subtle hover:bg-hover hover:text-fg'"
        :title="it.soon ? `${it.label} (coming in ${it.soon})` : it.label"
        :aria-label="it.label"
        :aria-current="view === it.id"
        @click="view = it.id"
      >
        <component :is="it.icon" :size="18" />
      </button>
      <button
        class="mt-auto flex size-9 items-center justify-center rounded-lg text-fg-subtle hover:bg-hover hover:text-fg"
        :title="`Theme: ${ui.theme}`"
        aria-label="Toggle theme"
        @click="ui.cycleTheme()"
      >
        <component :is="themeIcon[ui.theme]" :size="17" />
      </button>
    </nav>

    <!-- panel -->
    <div class="min-w-0 flex-1 bg-surface">
      <ConnectionsPanel v-if="view === 'connections'" />
      <div v-else class="flex h-full flex-col items-center justify-center px-6 text-center">
        <component :is="items.find((i) => i.id === view)!.icon" :size="28" class="text-fg-subtle" />
        <p class="mt-2 font-medium">{{ items.find((i) => i.id === view)!.label }}</p>
        <p class="mt-1 text-xs text-fg-muted">Coming in {{ items.find((i) => i.id === view)!.soon }}.</p>
      </div>
    </div>
  </div>
</template>
