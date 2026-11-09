import { defineStore } from "pinia";
import { reactive } from "vue";
import { sshApi, type ConnectionStatus, type SessionInfo } from "@/modules/ssh/api";
import type { HostKeyInfo } from "@/modules/connections/types";

export interface SessionState {
  status: ConnectionStatus;
  message?: string;
  connectedAt?: string;
  hostKey?: HostKeyInfo | null;
}

/** Live status of SSH connections, fed by backend `ssh://status` events. */
export const useSessionsStore = defineStore("sessions", () => {
  const byConnection = reactive<Record<string, SessionState>>({});
  let started = false;

  function status(connectionId: string): ConnectionStatus {
    return byConnection[connectionId]?.status ?? "disconnected";
  }

  function applySession(s: SessionInfo) {
    byConnection[s.connection_id] = {
      status: "connected",
      connectedAt: s.connected_at,
      hostKey: s.host_key,
    };
  }

  async function start() {
    if (started) return;
    started = true;
    await sshApi.onStatus((e) => {
      const prev = byConnection[e.connection_id];
      byConnection[e.connection_id] = {
        ...prev,
        status: e.status,
        message: e.message,
        connectedAt: e.status === "connected" ? new Date().toISOString() : prev?.connectedAt,
      };
      if (e.status === "connected") void refresh();
    });
    await refresh();
  }

  async function refresh() {
    try {
      const sessions = await sshApi.sessions();
      for (const s of sessions) applySession(s);
    } catch {
      /* non-fatal */
    }
  }

  async function disconnect(connectionId: string) {
    await sshApi.disconnect(connectionId);
  }

  return { byConnection, status, start, refresh, disconnect };
});
