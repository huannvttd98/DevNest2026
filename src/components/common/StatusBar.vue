<script setup lang="ts">
import { computed } from "vue";
import { ShieldAlert, ShieldCheck } from "lucide-vue-next";
import { useConnectionsStore } from "@/stores/connections";
import { useSessionsStore } from "@/stores/sessions";
import { useAppStore } from "@/stores/app";

const connections = useConnectionsStore();
const sessions = useSessionsStore();
const app = useAppStore();

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
    <span v-if="app.info" :title="app.info.data_dir">DevNest v{{ app.info.version }}</span>
  </footer>
</template>
