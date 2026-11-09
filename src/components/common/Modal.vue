<script lang="ts">
/** Open modals, top-most last: only the top one reacts to Escape. */
const stack: symbol[] = [];
</script>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { X } from "lucide-vue-next";

const props = withDefaults(defineProps<{ title: string; width?: string; dismissable?: boolean }>(), {
  width: "460px",
  dismissable: true,
});
const emit = defineEmits<{ close: [] }>();
const panel = ref<HTMLElement | null>(null);
const id = Symbol("modal");

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape" && props.dismissable && stack[stack.length - 1] === id) {
    e.stopPropagation();
    emit("close");
  }
}

onMounted(() => {
  stack.push(id);
  window.addEventListener("keydown", onKey, true);
  // Focus the first field (or the panel) for keyboard users.
  requestAnimationFrame(() => {
    const el = panel.value?.querySelector<HTMLElement>("[autofocus], input:not([type=hidden]), select, textarea, button.btn-primary");
    (el ?? panel.value)?.focus();
  });
});
onBeforeUnmount(() => {
  const i = stack.indexOf(id);
  if (i >= 0) stack.splice(i, 1);
  window.removeEventListener("keydown", onKey, true);
});
</script>

<template>
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4" @mousedown.self="dismissable && emit('close')">
    <div
      ref="panel"
      tabindex="-1"
      role="dialog"
      aria-modal="true"
      :aria-label="title"
      class="flex max-h-[90vh] w-full flex-col rounded-xl border border-line bg-surface shadow-2xl outline-none"
      :style="{ maxWidth: width }"
    >
      <header class="flex items-center justify-between border-b border-line px-4 py-3">
        <h2 class="text-sm font-semibold"><slot name="title">{{ title }}</slot></h2>
        <button v-if="dismissable" class="icon-btn" aria-label="Close" @click="emit('close')"><X :size="16" /></button>
      </header>
      <div class="overflow-y-auto px-4 py-4">
        <slot />
      </div>
      <footer v-if="$slots.footer" class="flex items-center justify-end gap-2 border-t border-line px-4 py-3">
        <slot name="footer" />
      </footer>
    </div>
  </div>
</template>
