<script setup lang="ts">
import { computed } from "vue";
import { CircleAlert, Download, LoaderCircle, Sparkles } from "lucide-vue-next";
import Modal from "@/components/common/Modal.vue";
import { useUpdaterStore } from "@/stores/updater";

const updater = useUpdaterStore();
const u = computed(() => updater.update);

const date = computed(() => {
  const d = u.value?.date ? new Date(u.value.date) : null;
  return d && !Number.isNaN(d.getTime()) ? d.toLocaleDateString() : null;
});

function fmtBytes(n: number) {
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

function close() {
  if (!updater.busy) updater.later();
}
</script>

<template>
  <Modal v-if="updater.dialogOpen && u" title="Update available" width="500px" :dismissable="!updater.busy" @close="close">
    <div class="space-y-4">
      <div class="flex items-start gap-3">
        <div class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent">
          <Sparkles :size="18" />
        </div>
        <div>
          <p class="font-medium">DevNest {{ u.version }} is available</p>
          <p class="text-xs text-fg-muted">
            You have {{ u.currentVersion }}<template v-if="date"> · released {{ date }}</template>
          </p>
        </div>
      </div>

      <div v-if="u.body" class="max-h-56 overflow-y-auto rounded-md border border-line bg-surface-2 p-3">
        <p class="mb-1 text-xs font-semibold text-fg-muted">What's new</p>
        <!-- Rendered as plain text on purpose (release notes are remote content). -->
        <p class="selectable text-xs leading-relaxed whitespace-pre-wrap">{{ u.body }}</p>
      </div>

      <div v-if="updater.status === 'downloading' || updater.status === 'installing'" class="space-y-1.5" role="status">
        <div class="h-1.5 overflow-hidden rounded-full bg-hover">
          <div
            class="h-full rounded-full bg-accent transition-[width] duration-200"
            :class="updater.progress === null ? 'w-1/3 animate-pulse' : ''"
            :style="updater.progress !== null ? { width: updater.progress + '%' } : undefined"
          />
        </div>
        <p class="text-xs text-fg-muted">
          <template v-if="updater.status === 'installing'">Installing… DevNest will restart.</template>
          <template v-else>
            Downloading… {{ fmtBytes(updater.downloaded) }}
            <template v-if="updater.total"> / {{ fmtBytes(updater.total) }} ({{ updater.progress }}%)</template>
          </template>
        </p>
      </div>

      <p v-if="updater.status === 'error' && updater.error" class="selectable flex gap-2 rounded-md bg-danger-soft px-3 py-2 text-xs text-danger" role="alert">
        <CircleAlert :size="14" class="mt-0.5 shrink-0" /> Update failed: {{ updater.error }}
      </p>

      <p class="text-xs text-fg-subtle">Your connections, keys and settings are kept. The download is verified with DevNest's signing key before it is installed.</p>
    </div>

    <template #footer>
      <button class="btn" :disabled="updater.busy" @click="updater.later()">Later</button>
      <button class="btn btn-primary" :disabled="updater.busy" @click="updater.install()">
        <LoaderCircle v-if="updater.busy" :size="14" class="animate-spin" />
        <Download v-else :size="14" />
        {{ updater.status === "error" ? "Try again" : "Update & restart" }}
      </button>
    </template>
  </Modal>
</template>
