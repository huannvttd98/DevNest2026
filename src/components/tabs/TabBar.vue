<script setup lang="ts">
import { nextTick, ref } from "vue";
import { LoaderCircle, PanelRight, PanelRightClose, SquareTerminal, Unplug, X } from "lucide-vue-next";
import ContextMenu, { type MenuItem } from "@/components/common/ContextMenu.vue";
import { useTabsStore, type Tab } from "@/stores/tabs";
import { useUiStore } from "@/stores/ui";

const tabs = useTabsStore();
const ui = useUiStore();
const editingId = ref<string | null>(null);
const editValue = ref("");
const editInput = ref<HTMLInputElement[] | null>(null);
const menu = ref<{ x: number; y: number; items: MenuItem[] } | null>(null);

function startRename(t: Tab) {
  editingId.value = t.id;
  editValue.value = t.title;
  void nextTick(() => editInput.value?.[0]?.select());
}

function commitRename() {
  if (editingId.value) tabs.rename(editingId.value, editValue.value);
  editingId.value = null;
}

function tabMenu(e: MouseEvent, t: Tab) {
  menu.value = {
    x: e.clientX,
    y: e.clientY,
    items: [
      { label: "Rename…", action: () => startRename(t) },
      { separator: true },
      { label: "Close", action: () => tabs.close(t.id) },
      { label: "Close others", action: () => tabs.tabs.filter((x) => x.id !== t.id).forEach((x) => tabs.close(x.id)) },
      { label: "Close all", action: () => [...tabs.tabs].forEach((x) => tabs.close(x.id)) },
    ],
  };
}
</script>

<template>
  <div class="flex h-9 shrink-0 items-stretch border-b border-line bg-surface-2">
    <div class="flex min-w-0 flex-1 items-stretch overflow-x-auto" role="tablist" aria-label="Open tabs">
      <div
        v-for="t in tabs.tabs"
        :key="t.id"
        role="tab"
        tabindex="0"
        :aria-selected="t.id === tabs.activeId"
        class="group relative flex max-w-[220px] min-w-[120px] cursor-pointer items-center gap-1.5 border-r border-line pr-1 pl-3 text-[13px]"
        :class="t.id === tabs.activeId ? 'bg-surface text-fg' : 'text-fg-muted hover:bg-hover'"
        :title="t.message ? `${t.title} — ${t.message}` : t.title"
        @click="tabs.activate(t.id)"
        @auxclick.middle="tabs.close(t.id)"
        @dblclick="startRename(t)"
        @keydown.enter="tabs.activate(t.id)"
        @keydown.f2="startRename(t)"
        @contextmenu.prevent="tabMenu($event, t)"
      >
        <span v-if="t.id === tabs.activeId" class="absolute inset-x-0 top-0 h-0.5 bg-accent" />
        <LoaderCircle v-if="t.state === 'connecting'" :size="13" class="shrink-0 animate-spin text-warn" />
        <Unplug v-else-if="t.state === 'closed' || t.state === 'error'" :size="13" class="shrink-0" :class="t.state === 'error' ? 'text-danger' : 'text-fg-subtle'" />
        <SquareTerminal v-else :size="13" class="shrink-0 text-ok" />
        <input
          v-if="editingId === t.id"
          ref="editInput"
          v-model="editValue"
          class="h-6 min-w-0 flex-1 rounded border border-accent bg-surface px-1 text-[13px] outline-none"
          aria-label="Tab name"
          maxlength="60"
          @keydown.enter.stop="commitRename"
          @keydown.esc.stop="editingId = null"
          @blur="commitRename"
          @click.stop
        />
        <span v-else class="min-w-0 flex-1 truncate">{{ t.title }}</span>
        <button
          class="icon-btn size-5 shrink-0 opacity-0 group-hover:opacity-100 focus:opacity-100"
          :class="t.id === tabs.activeId ? 'opacity-100' : ''"
          aria-label="Close tab"
          title="Close (Ctrl+Shift+W)"
          @click.stop="tabs.close(t.id)"
        >
          <X :size="13" />
        </button>
      </div>
    </div>
    <button
      class="icon-btn m-1 shrink-0"
      :title="ui.infoPanelOpen ? 'Hide info panel (Ctrl+Alt+I)' : 'Show info panel (Ctrl+Alt+I)'"
      aria-label="Toggle info panel"
      @click="ui.infoPanelOpen = !ui.infoPanelOpen"
    >
      <component :is="ui.infoPanelOpen ? PanelRightClose : PanelRight" :size="16" />
    </button>
    <ContextMenu v-if="menu" :x="menu.x" :y="menu.y" :items="menu.items" @close="menu = null" />
  </div>
</template>
