<script setup lang="ts">
import { onMounted } from "vue";
import AppLayout from "@/layouts/AppLayout.vue";
import ConnectionDialog from "@/components/connections/ConnectionDialog.vue";
import DialogHost from "@/components/dialogs/DialogHost.vue";
import Toasts from "@/components/common/Toasts.vue";
import UpdateDialog from "@/components/common/UpdateDialog.vue";
import { useUiStore } from "@/stores/ui";
import { useConnectionsStore } from "@/stores/connections";
import { useSessionsStore } from "@/stores/sessions";
import { useAppStore } from "@/stores/app";
import { useToastStore } from "@/stores/toasts";
import { useUpdaterStore } from "@/stores/updater";

const ui = useUiStore();
const connections = useConnectionsStore();
const sessions = useSessionsStore();
const app = useAppStore();
const toasts = useToastStore();
const updater = useUpdaterStore();

onMounted(async () => {
  void sessions.start();
  updater.start();
  await connections.load();
  const info = await app.load();
  if (info?.vault_error) {
    toasts.error("Secure storage unavailable", `Saved passwords and keys cannot be used: ${info.vault_error}`);
  } else if (info?.vault_state === "created" && connections.connections.some((c) => c.has_password || c.has_passphrase || c.has_private_key)) {
    toasts.warning(
      "Vault key was recreated",
      "The previous vault key was not found in the OS keychain. Saved secrets must be entered again.",
    );
  }
});

// Block the WebView's default context menu (keep it for text fields).
document.addEventListener("contextmenu", (e) => {
  const t = e.target as HTMLElement;
  if (!t.closest("input, textarea, .selectable")) e.preventDefault();
});
</script>

<template>
  <AppLayout />
  <ConnectionDialog
    v-if="ui.editor"
    :connection="ui.editor.connection"
    :default-group-id="ui.editor.groupId"
    @close="ui.closeEditor()"
  />
  <UpdateDialog />
  <DialogHost />
  <Toasts />
</template>
