<script setup lang="ts">
import { ref } from "vue";
import {
  ChevronDown,
  ChevronRight,
  Folder,
  FolderInput,
  FolderPlus,
  MoreHorizontal,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Server,
  SquareTerminal,
  Trash2,
  Unplug,
} from "lucide-vue-next";
import StatusDot from "@/components/common/StatusDot.vue";
import ContextMenu, { type MenuItem } from "@/components/common/ContextMenu.vue";
import { useConnectionsStore } from "@/stores/connections";
import { useSessionsStore } from "@/stores/sessions";
import { useTabsStore } from "@/stores/tabs";
import { useConnectionActions } from "@/modules/connections/actions";
import type { Connection, Group } from "@/modules/connections/types";

const store = useConnectionsStore();
const sessions = useSessionsStore();
const tabs = useTabsStore();
const actions = useConnectionActions();

const menu = ref<{ x: number; y: number; items: MenuItem[] } | null>(null);
const selectedId = ref<string | null>(null);

function connectionMenu(e: MouseEvent, c: Connection) {
  selectedId.value = c.id;
  const connected = sessions.status(c.id) === "connected";
  const moveTargets: MenuItem[] = [
    ...(c.group_id ? [{ label: "Move to: Ungrouped", icon: FolderInput, action: () => actions.move(c, null) }] : []),
    ...store.groups
      .filter((g) => g.id !== c.group_id)
      .map((g) => ({ label: `Move to: ${g.name}`, icon: FolderInput, action: () => actions.move(c, g.id) })),
  ];
  menu.value = {
    x: e.clientX,
    y: e.clientY,
    items: [
      { label: "Open terminal", icon: SquareTerminal, action: () => actions.openTerminal(c) },
      { label: "Disconnect", icon: Unplug, disabled: !connected, action: () => actions.disconnect(c) },
      { separator: true },
      { label: "Edit…", icon: Pencil, action: () => actions.edit(c) },
      ...(moveTargets.length ? [{ separator: true }, ...moveTargets] : []),
      { separator: true },
      { label: "Delete…", icon: Trash2, danger: true, action: () => actions.remove(c) },
    ],
  };
}

function groupMenu(e: MouseEvent, g: Group) {
  menu.value = {
    x: e.clientX,
    y: e.clientY,
    items: [
      { label: "New connection here…", icon: Plus, action: () => actions.newConnection(g.id) },
      { label: "Rename…", icon: Pencil, action: () => actions.renameGroup(g) },
      { separator: true },
      { label: "Delete group…", icon: Trash2, danger: true, action: () => actions.deleteGroup(g) },
    ],
  };
}

function openCount(id: string) {
  return tabs.tabs.filter((t) => t.connectionId === id).length;
}

function onKey(e: KeyboardEvent, c: Connection) {
  if (e.key === "Enter") actions.openTerminal(c);
  else if (e.key === "Delete") actions.remove(c);
  else if (e.key === "F2") actions.edit(c);
}
</script>

<template>
  <div class="flex h-full flex-col">
    <header class="flex h-10 shrink-0 items-center justify-between pr-1.5 pl-3">
      <h2 class="text-[11px] font-semibold tracking-wider text-fg-muted uppercase">Connections</h2>
      <div class="flex">
        <button class="icon-btn" title="New connection (Ctrl+Shift+N)" aria-label="New connection" @click="actions.newConnection()">
          <Plus :size="16" />
        </button>
        <button class="icon-btn" title="New group" aria-label="New group" @click="actions.newGroup()"><FolderPlus :size="15" /></button>
        <button class="icon-btn" title="Reload" aria-label="Reload connections" @click="store.load()">
          <RefreshCw :size="14" :class="store.loading ? 'animate-spin' : ''" />
        </button>
      </div>
    </header>

    <div class="px-2 pb-2">
      <div class="relative">
        <Search :size="13" class="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-fg-subtle" />
        <input v-model="store.filter" class="field h-7 pl-7 text-xs" placeholder="Filter connections" aria-label="Filter connections" />
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-1.5 pb-3" role="tree" aria-label="Connections">
      <!-- loading -->
      <div v-if="store.loading && !store.loaded" class="space-y-2 px-2 pt-1">
        <div v-for="i in 4" :key="i" class="h-6 animate-pulse rounded bg-hover" />
      </div>

      <!-- error -->
      <div v-else-if="store.error" class="mx-1 rounded-md bg-danger-soft p-3 text-xs text-danger">
        <p class="font-medium">Could not load connections</p>
        <p class="selectable mt-1 break-words">{{ store.error }}</p>
        <button class="btn mt-2 h-7 text-xs" @click="store.load()">Retry</button>
      </div>

      <!-- empty -->
      <div v-else-if="!store.connections.length && !store.groups.length" class="px-3 pt-6 text-center">
        <Server :size="28" class="mx-auto text-fg-subtle" />
        <p class="mt-2 font-medium">No connections yet</p>
        <p class="mt-1 text-xs text-fg-muted">Add your first SSH server to get started.</p>
        <button class="btn btn-primary mt-3" @click="actions.newConnection()"><Plus :size="14" /> New connection</button>
      </div>

      <p v-else-if="store.filter && !store.tree.length" class="px-3 pt-4 text-center text-xs text-fg-muted">No matches for “{{ store.filter }}”.</p>

      <template v-else>
        <div v-for="node in store.tree" :key="node.group?.id ?? '__ungrouped'" role="group" class="mb-0.5">
          <!-- group header -->
          <div
            v-if="node.group || store.groups.length"
            class="group flex h-7 cursor-pointer items-center gap-1 rounded-md px-1.5 text-xs font-medium text-fg-muted hover:bg-hover"
            role="treeitem"
            :aria-expanded="!node.group || !store.collapsed.has(node.group.id)"
            tabindex="0"
            @click="node.group && store.toggleGroup(node.group.id)"
            @keydown.enter="node.group && store.toggleGroup(node.group.id)"
            @contextmenu.prevent="node.group && groupMenu($event, node.group)"
          >
            <component
              :is="node.group && store.collapsed.has(node.group.id) && !store.filter ? ChevronRight : ChevronDown"
              :size="14"
              class="shrink-0"
            />
            <Folder :size="13" class="shrink-0" />
            <span class="truncate">{{ node.group?.name ?? "Ungrouped" }}</span>
            <span class="ml-1 text-fg-subtle">{{ node.connections.length }}</span>
            <button
              v-if="node.group"
              class="icon-btn ml-auto size-5 opacity-0 group-hover:opacity-100 focus:opacity-100"
              aria-label="Group actions"
              @click.stop="groupMenu($event, node.group)"
            >
              <MoreHorizontal :size="14" />
            </button>
          </div>

          <!-- connections -->
          <template v-if="!node.group || !store.collapsed.has(node.group.id) || store.filter">
            <div
              v-for="c in node.connections"
              :key="c.id"
              role="treeitem"
              tabindex="0"
              class="group flex h-8 cursor-pointer items-center gap-2 rounded-md pr-1 text-[13px] hover:bg-hover"
              :class="[store.groups.length ? 'pl-6' : 'pl-2', selectedId === c.id ? 'bg-hover' : '']"
              :title="`${c.username}@${c.host}:${c.port}`"
              @click="selectedId = c.id"
              @dblclick="actions.openTerminal(c)"
              @keydown="onKey($event, c)"
              @contextmenu.prevent="connectionMenu($event, c)"
            >
              <StatusDot :status="sessions.status(c.id)" />
              <div class="min-w-0 flex-1 leading-tight">
                <p class="truncate">{{ c.name }}</p>
                <p class="truncate text-[11px] text-fg-subtle">{{ c.username }}@{{ c.host }}</p>
              </div>
              <span v-if="openCount(c.id)" class="rounded bg-accent-soft px-1 text-[10px] font-medium text-accent" :title="`${openCount(c.id)} open terminal(s)`">
                {{ openCount(c.id) }}
              </span>
              <button
                class="icon-btn size-6 opacity-0 group-hover:opacity-100 focus:opacity-100"
                title="Open terminal"
                aria-label="Open terminal"
                @click.stop="actions.openTerminal(c)"
              >
                <SquareTerminal :size="14" />
              </button>
              <button
                class="icon-btn size-6 opacity-0 group-hover:opacity-100 focus:opacity-100"
                aria-label="Connection actions"
                @click.stop="connectionMenu($event, c)"
              >
                <MoreHorizontal :size="14" />
              </button>
            </div>
            <p v-if="node.group && !node.connections.length" class="py-1 pl-7 text-[11px] text-fg-subtle">Empty group</p>
          </template>
        </div>
      </template>
    </div>

    <ContextMenu v-if="menu" :x="menu.x" :y="menu.y" :items="menu.items" @close="menu = null" />
  </div>
</template>
