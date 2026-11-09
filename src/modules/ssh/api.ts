import { Channel } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { call } from "@/lib/ipc";
import type { CredentialOverrides, HostKeyInfo } from "@/modules/connections/types";

export type ConnectionStatus = "connecting" | "connected" | "disconnected" | "error";

export interface StatusEvent {
  connection_id: string;
  status: ConnectionStatus;
  message?: string;
}

export interface SessionInfo {
  connection_id: string;
  connected_at: string;
  host_key: HostKeyInfo | null;
  terminals: number;
}

export type TerminalEvent =
  | { type: "exit"; code: number | null; signal: string | null }
  | { type: "closed"; reason: string | null };

export interface TerminalHandlers {
  onData: (bytes: Uint8Array) => void;
  onEvent: (event: TerminalEvent) => void;
}

/** Output arrives as raw bytes (ArrayBuffer); control events arrive as JSON objects. */
export function createTerminalChannel(h: TerminalHandlers): Channel<unknown> {
  return new Channel<unknown>((msg) => {
    if (msg instanceof ArrayBuffer) h.onData(new Uint8Array(msg));
    else if (msg instanceof Uint8Array) h.onData(msg);
    else if (Array.isArray(msg)) h.onData(Uint8Array.from(msg as number[]));
    else if (msg && typeof msg === "object" && "type" in msg) h.onEvent(msg as TerminalEvent);
  });
}

export const sshApi = {
  sessions: () => call<SessionInfo[]>("ssh_sessions"),
  disconnect: (connectionId: string) => call<void>("ssh_disconnect", { connectionId }),

  openTerminal: (args: {
    connectionId: string;
    cols: number;
    rows: number;
    overrides?: CredentialOverrides;
    saveOverrides?: boolean;
    channel: Channel<unknown>;
  }) =>
    call<{ terminal_id: string; session: SessionInfo | null }>("terminal_open", {
      connectionId: args.connectionId,
      cols: args.cols,
      rows: args.rows,
      overrides: args.overrides ?? null,
      saveOverrides: args.saveOverrides ?? false,
      onEvent: args.channel,
    }),

  write: (terminalId: string, data: string, binary = false) =>
    call<void>("terminal_write", { terminalId, data, binary }),
  resize: (terminalId: string, cols: number, rows: number) =>
    call<void>("terminal_resize", { terminalId, cols, rows }),
  close: (terminalId: string) => call<void>("terminal_close", { terminalId }),

  onStatus: (cb: (e: StatusEvent) => void): Promise<UnlistenFn> =>
    listen<StatusEvent>("ssh://status", (e) => cb(e.payload)),
};
