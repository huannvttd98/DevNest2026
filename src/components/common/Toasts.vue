<script setup lang="ts">
import { CircleAlert, CircleCheck, Info, TriangleAlert, X } from "lucide-vue-next";
import { useToastStore } from "@/stores/toasts";

const toasts = useToastStore();
const icons = { info: Info, success: CircleCheck, error: CircleAlert, warning: TriangleAlert };
const colors = { info: "text-accent", success: "text-ok", error: "text-danger", warning: "text-warn" };
</script>

<template>
  <div class="pointer-events-none fixed right-4 bottom-10 z-[60] flex w-80 flex-col gap-2" aria-live="polite">
    <TransitionGroup
      enter-from-class="opacity-0 translate-y-2"
      leave-to-class="opacity-0"
      enter-active-class="transition duration-150"
      leave-active-class="transition duration-150"
    >
      <div
        v-for="t in toasts.items"
        :key="t.id"
        class="pointer-events-auto flex gap-2.5 rounded-lg border border-line bg-surface p-3 shadow-lg"
      >
        <component :is="icons[t.kind]" :size="16" class="mt-0.5 shrink-0" :class="colors[t.kind]" />
        <div class="min-w-0 flex-1">
          <p class="font-medium">{{ t.title }}</p>
          <p v-if="t.message" class="selectable mt-0.5 break-words text-xs text-fg-muted">{{ t.message }}</p>
        </div>
        <button class="icon-btn -mt-1 -mr-1 size-6" aria-label="Dismiss" @click="toasts.dismiss(t.id)"><X :size="14" /></button>
      </div>
    </TransitionGroup>
  </div>
</template>
