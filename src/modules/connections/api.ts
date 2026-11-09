import { call } from "@/lib/ipc";
import type { Connection, CredentialOverrides, Group, HostKeyInfo, SaveConnectionInput } from "./types";

export const connectionsApi = {
  listGroups: () => call<Group[]>("groups_list"),
  createGroup: (name: string) => call<Group>("group_create", { name }),
  renameGroup: (id: string, name: string) => call<Group>("group_rename", { id, name }),
  deleteGroup: (id: string) => call<void>("group_delete", { id }),

  list: () => call<Connection[]>("connections_list"),
  create: (input: SaveConnectionInput) => call<Connection>("connection_create", { input }),
  update: (id: string, input: SaveConnectionInput) => call<Connection>("connection_update", { id, input }),
  move: (id: string, groupId: string | null) => call<Connection>("connection_move", { id, groupId }),
  remove: (id: string) => call<void>("connection_delete", { id }),

  test: (connectionId: string | null, input: SaveConnectionInput, overrides?: CredentialOverrides) =>
    call<{ elapsed_ms: number; host_key: HostKeyInfo | null }>("ssh_test", {
      connectionId,
      input,
      overrides: overrides ?? null,
    }),

  trustHostKey: (key: HostKeyInfo, replace: boolean) =>
    call<void>("known_host_trust", {
      key: { host: key.host, port: key.port, key_type: key.key_type, fingerprint: key.fingerprint },
      replace,
    }),
};
