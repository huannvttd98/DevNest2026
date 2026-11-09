<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import type { Component } from "vue";

export interface MenuItem {
  label?: string;
  icon?: Component;
  danger?: boolean;
  disabled?: boolean;
  separator?: boolean;
  action?: () => void;
}

const props = defineProps<{ x: number; y: number; items: MenuItem[] }>();
const emit = defineEmits<{ close: [] }>();
const el = ref<HTMLElement | null>(null);
const pos = ref({ left: props.x, top: props.y });

function onDown(e: MouseEvent) {
  if (el.value && !el.value.contains(e.target as Node)) emit("close");
}
function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}

onMounted(async () => {
  await nextTick();
  // Keep the menu inside the window.
  const r = el.value?.getBoundingClientRect();
  if (r) {
    pos.value = {
      left: Math.min(props.x, window.innerWidth - r.width - 4),
      top: Math.min(props.y, window.innerHeight - r.height - 4),
    };
  }
  el.value?.querySelector<HTMLElement>("button:not(:disabled)")?.focus();
  window.addEventListener("mousedown", onDown, true);
  window.addEventListener("keydown", onKey, true);
  window.addEventListener("blur", () => emit("close"), { once: true });
});
onBeforeUnmount(() => {
  window.removeEventListener("mousedown", onDown, true);
  window.removeEventListener("keydown", onKey, true);
});

function run(item: MenuItem) {
  emit("close");
  item.action?.();
}
</script>

<template>
  <Teleport to="body">
    <div
      ref="el"
      role="menu"
      class="fixed z-[70] min-w-[190px] rounded-lg border border-line bg-surface py-1 shadow-xl"
      :style="{ left: pos.left + 'px', top: pos.top + 'px' }"
    >
      <template v-for="(item, i) in items" :key="i">
        <div v-if="item.separator" class="my-1 h-px bg-line" />
        <button
          v-else
          role="menuitem"
          class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-[13px] hover:bg-hover focus:bg-hover focus:outline-none disabled:opacity-40"
          :class="item.danger ? 'text-danger' : 'text-fg'"
          :disabled="item.disabled"
          @click="run(item)"
        >
          <component :is="item.icon" v-if="item.icon" :size="14" class="shrink-0 opacity-80" />
          <span v-else class="w-3.5" />
          {{ item.label }}
        </button>
      </template>
    </div>
  </Teleport>
</template>
