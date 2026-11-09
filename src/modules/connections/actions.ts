import { useConnectionsStore } from "@/stores/connections";
import { useDialogStore } from "@/stores/dialogs";
import { useSessionsStore } from "@/stores/sessions";
import { useTabsStore } from "@/stores/tabs";
import { useToastStore } from "@/stores/toasts";
import { useUiStore } from "@/stores/ui";
import { errorMessage } from "@/lib/ipc";
import type { Connection, Group } from "./types";

/** User-facing connection actions shared by the sidebar, context menus and shortcuts. */
export function useConnectionActions() {
  const store = useConnectionsStore();
  const dialogs = useDialogStore();
  const sessions = useSessionsStore();
  const tabs = useTabsStore();
  const toasts = useToastStore();
  const ui = useUiStore();

  return {
    openTerminal(c: Connection) {
      tabs.openTerminal(c);
    },
    newConnection(groupId: string | null = null) {
      ui.openEditor(null, groupId);
    },
    edit(c: Connection) {
      ui.openEditor(c);
    },
    async remove(c: Connection) {
      const ok = await dialogs.confirm({
        title: "Delete connection",
        message: `Delete "${c.name}" (${c.username}@${c.host})?\nIts saved password / key / passphrase are removed from the vault. Open terminals will be closed.`,
        confirmText: "Delete",
        danger: true,
      });
      if (!ok) return;
      try {
        tabs.closeForConnection(c.id);
        await store.remove(c.id);
        toasts.success("Connection deleted", c.name);
      } catch (e) {
        toasts.error("Could not delete connection", errorMessage(e));
      }
    },
    async disconnect(c: Connection) {
      try {
        await sessions.disconnect(c.id);
      } catch (e) {
        toasts.error("Disconnect failed", errorMessage(e));
      }
    },
    async move(c: Connection, groupId: string | null) {
      try {
        await store.move(c.id, groupId);
      } catch (e) {
        toasts.error("Could not move connection", errorMessage(e));
      }
    },
    async newGroup() {
      const name = await dialogs.input({ title: "New group", label: "Group name", value: "", placeholder: "e.g. Production", confirmText: "Create" });
      if (!name) return;
      try {
        await store.createGroup(name);
      } catch (e) {
        toasts.error("Could not create group", errorMessage(e));
      }
    },
    async renameGroup(g: Group) {
      const name = await dialogs.input({ title: "Rename group", label: "Group name", value: g.name });
      if (!name || name === g.name) return;
      try {
        await store.renameGroup(g.id, name);
      } catch (e) {
        toasts.error("Could not rename group", errorMessage(e));
      }
    },
    async deleteGroup(g: Group) {
      const count = store.connections.filter((c) => c.group_id === g.id).length;
      const ok = await dialogs.confirm({
        title: "Delete group",
        message: count
          ? `Delete group "${g.name}"? Its ${count} connection(s) will be kept and moved to "Ungrouped".`
          : `Delete group "${g.name}"?`,
        confirmText: "Delete",
        danger: true,
      });
      if (!ok) return;
      try {
        await store.deleteGroup(g.id);
      } catch (e) {
        toasts.error("Could not delete group", errorMessage(e));
      }
    },
  };
}
