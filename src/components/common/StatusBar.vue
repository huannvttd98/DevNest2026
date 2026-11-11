<script setup lang="ts">
import { computed } from "vue";
import { ArrowUpCircle, RefreshCw, ShieldAlert, ShieldCheck } from "lucide-vue-next";
import { useConnectionsStore } from "@/stores/connections";
import { useSessionsStore } from "@/stores/sessions";
import { useAppStore } from "@/stores/app";
import { useUpdaterStore } from "@/stores/updater";

const connections = useConnectionsStore();
const sessions = useSessionsStore();
const app = useAppStore();
const updater = useUpdaterStore();

const live = computed(() => Object.values(sessions.byConnection).filter((s) => s.status === "connected").length);
</script>

<template>
  <footer class="flex h-6 shrink-0 items-center gap-4 border-t border-line bg-surface-2 px-3 text-[11px] text-fg-muted">
    <span>{{ connections.connections.length }} connection(s)</span>
    <span class="flex items-center gap-1.5">
      <span class="size-1.5 rounded-full" :class="live ? 'bg-ok' : 'bg-fg-subtle/50'" />
      {{ live }} active session(s)
    </span>
    <span class="ml-auto flex items-center gap-1" :title="app.info?.vault_error ?? 'Secrets are encrypted with a key stored in the OS keychain'">
      <template v-if="app.info?.vault_error">
        <ShieldAlert :size="12" class="text-danger" /> <span class="text-danger">Vault unavailable</span>
      </template>
      <template v-else-if="app.info">
        <ShieldCheck :size="12" class="text-ok" /> Vault ready
      </template>
    </span>
    <button
      v-if="updater.update && (updater.status === 'available' || updater.busy || updater.status === 'error')"
      class="flex items-center gap-1 rounded px-1.5 font-medium text-accent hover:bg-hover"
      :title="`Update to ${updater.update.version}`"
      @click="updater.dialogOpen = true"
    >
      <ArrowUpCircle :size="12" /> Update {{ updater.update.version }}
    </button>
    <button
      v-else
      class="flex items-center gap-1 rounded px-1 hover:bg-hover hover:text-fg"
      :title="updater.lastChecked ? `Last checked ${updater.lastChecked.toLocaleTimeString()}` : 'Check for updates'"
      aria-label="Check for updates"
      :disabled="updater.status === 'checking'"
      @click="updater.checkForUpdates(true)"
    >
      <RefreshCw :size="11" :class="updater.status === 'checking' ? 'animate-spin' : ''" />
    </button>
    <span v-if="app.info" :title="app.info.data_dir">DevNest v{{ app.info.version }}</span>
  </footer>
</template>
