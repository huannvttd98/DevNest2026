<script setup lang="ts">
import { computed } from "vue";
import { Info, Pencil, Unplug } from "lucide-vue-next";
import StatusDot from "@/components/common/StatusDot.vue";
import { useConnectionsStore } from "@/stores/connections";
import { useSessionsStore } from "@/stores/sessions";
import { useTabsStore } from "@/stores/tabs";
import { useConnectionActions } from "@/modules/connections/actions";

const tabs = useTabsStore();
const connections = useConnectionsStore();
const sessions = useSessionsStore();
const actions = useConnectionActions();

const tab = computed(() => tabs.active);
const conn = computed(() => (tab.value ? connections.byId.get(tab.value.connectionId) : undefined));
const session = computed(() => (conn.value ? sessions.byConnection[conn.value.id] : undefined));
const status = computed(() => (conn.value ? sessions.status(conn.value.id) : "disconnected"));
const group = computed(() => connections.groups.find((g) => g.id === conn.value?.group_id));

const authLabel = computed(() => {
  const c = conn.value;
  if (!c) return "";
  if (c.auth_type === "password") return c.has_password ? "Password (saved)" : "Password (ask)";
  return c.key_source === "vault" ? "Private key (vault)" : "Private key (file)";
});

function fmt(ts?: string | null) {
  if (!ts) return "—";
  const d = new Date(ts);
  return Number.isNaN(d.getTime()) ? ts : d.toLocaleString();
}
</script>

<template>
  <aside class="flex h-full flex-col border-l border-line bg-surface" aria-label="Information">
    <header class="flex h-9 shrink-0 items-center border-b border-line px-3">
      <h2 class="text-[11px] font-semibold tracking-wider text-fg-muted uppercase">Information</h2>
    </header>

    <div v-if="!conn" class="flex flex-1 flex-col items-center justify-center px-6 text-center text-xs text-fg-muted">
      <Info :size="22" class="mb-2 text-fg-subtle" />
      Open a terminal to see details about the server here.
    </div>

    <div v-else class="selectable flex-1 space-y-5 overflow-y-auto p-3 text-xs">
      <section>
        <p class="truncate text-sm font-semibold">{{ conn.name }}</p>
        <p class="mt-0.5 flex items-center gap-1.5 text-fg-muted">
          <StatusDot :status="status" />
          <span class="capitalize">{{ status }}</span>
        </p>
        <p v-if="status === 'error' && session?.message" class="mt-1 break-words text-danger">{{ session.message }}</p>
      </section>

      <section>
        <h3 class="mb-1.5 font-semibold text-fg-muted">Server</h3>
        <dl class="grid grid-cols-[84px_1fr] gap-x-2 gap-y-1">
          <dt class="text-fg-subtle">Host</dt>
          <dd class="font-mono break-all">{{ conn.host }}</dd>
          <dt class="text-fg-subtle">Port</dt>
          <dd class="font-mono">{{ conn.port }}</dd>
          <dt class="text-fg-subtle">Username</dt>
          <dd class="font-mono break-all">{{ conn.username }}</dd>
          <dt class="text-fg-subtle">Auth</dt>
          <dd>{{ authLabel }}</dd>
          <template v-if="conn.key_source === 'path'">
            <dt class="text-fg-subtle">Key file</dt>
            <dd class="font-mono break-all">{{ conn.key_path }}</dd>
          </template>
          <dt class="text-fg-subtle">Group</dt>
          <dd>{{ group?.name ?? "—" }}</dd>
        </dl>
      </section>

      <section>
        <h3 class="mb-1.5 font-semibold text-fg-muted">Session</h3>
        <dl class="grid grid-cols-[84px_1fr] gap-x-2 gap-y-1">
          <dt class="text-fg-subtle">Connected</dt>
          <dd>{{ status === "connected" ? fmt(session?.connectedAt) : "—" }}</dd>
          <dt class="text-fg-subtle">Last used</dt>
          <dd>{{ fmt(conn.last_connected_at) }}</dd>
          <dt class="text-fg-subtle">Terminals</dt>
          <dd>{{ tabs.tabs.filter((t) => t.connectionId === conn!.id).length }}</dd>
          <template v-if="session?.hostKey">
            <dt class="text-fg-subtle">Host key</dt>
            <dd class="font-mono break-all">
              {{ session.hostKey.key_type }}<br />
              <span class="text-fg-muted">{{ session.hostKey.fingerprint }}</span>
            </dd>
          </template>
        </dl>
      </section>

      <section class="flex flex-wrap gap-2">
        <button class="btn h-7 text-xs" @click="actions.edit(conn)"><Pencil :size="12" /> Edit</button>
        <button class="btn h-7 text-xs" :disabled="status !== 'connected'" @click="actions.disconnect(conn)">
          <Unplug :size="12" /> Disconnect
        </button>
      </section>
    </div>
  </aside>
</template>
