<script setup lang="ts">
import { ref, watch } from "vue";
import { KeyRound, ShieldAlert, ShieldQuestion } from "lucide-vue-next";
import Modal from "@/components/common/Modal.vue";
import { useDialogStore, type DialogRequest } from "@/stores/dialogs";

const dialogs = useDialogStore();
const text = ref("");
const save = ref(false);
const changedAck = ref(false);

watch(
  () => dialogs.current,
  (d) => {
    text.value = d?.kind === "input" ? d.value : "";
    save.value = false;
    changedAck.value = false;
  },
);

function cancel() {
  const d = dialogs.current;
  if (!d) return;
  dialogs.close();
  resolveCancel(d);
}

function resolveCancel(d: DialogRequest) {
  switch (d.kind) {
    case "confirm":
    case "hostKey":
      return d.resolve(false);
    case "input":
    case "credential":
      return d.resolve(null);
  }
}

function accept() {
  const d = dialogs.current;
  if (!d) return;
  if (d.kind === "input" && !text.value.trim()) return;
  if (d.kind === "credential" && !text.value) return;
  if (d.kind === "hostKey" && d.changed && !changedAck.value) return;
  dialogs.close();
  switch (d.kind) {
    case "confirm":
    case "hostKey":
      return d.resolve(true);
    case "input":
      return d.resolve(text.value.trim());
    case "credential":
      return d.resolve({ value: text.value, save: save.value });
  }
}
</script>

<template>
  <template v-if="dialogs.current">
    <!-- Confirm -->
    <Modal v-if="dialogs.current.kind === 'confirm'" :title="dialogs.current.title" @close="cancel">
      <p class="selectable whitespace-pre-line text-fg-muted">{{ dialogs.current.message }}</p>
      <template #footer>
        <button class="btn" @click="cancel">Cancel</button>
        <button class="btn" :class="dialogs.current.danger ? 'btn-danger' : 'btn-primary'" @click="accept">
          {{ dialogs.current.confirmText ?? "OK" }}
        </button>
      </template>
    </Modal>

    <!-- Text input -->
    <Modal v-else-if="dialogs.current.kind === 'input'" :title="dialogs.current.title" width="400px" @close="cancel">
      <form @submit.prevent="accept">
        <label class="label" for="dlg-input">{{ dialogs.current.label }}</label>
        <input id="dlg-input" v-model="text" class="field" :placeholder="dialogs.current.placeholder" maxlength="100" autofocus />
      </form>
      <template #footer>
        <button class="btn" @click="cancel">Cancel</button>
        <button class="btn btn-primary" :disabled="!text.trim()" @click="accept">{{ dialogs.current.confirmText ?? "Save" }}</button>
      </template>
    </Modal>

    <!-- Password / passphrase prompt -->
    <Modal
      v-else-if="dialogs.current.kind === 'credential'"
      :title="dialogs.current.secret === 'password' ? 'Password required' : 'Key passphrase required'"
      width="420px"
      @close="cancel"
    >
      <form class="space-y-3" @submit.prevent="accept" @keydown.enter.prevent="accept">
        <div class="flex items-start gap-3">
          <KeyRound :size="20" class="mt-0.5 shrink-0 text-accent" />
          <p class="text-fg-muted">
            <template v-if="dialogs.current.secret === 'password'">
              Enter the password for
              <span class="font-medium text-fg">{{ dialogs.current.username }}@{{ dialogs.current.host }}</span>
              ({{ dialogs.current.connectionName }}).
            </template>
            <template v-else>
              The private key of <span class="font-medium text-fg">{{ dialogs.current.connectionName }}</span> is protected by a passphrase.
            </template>
          </p>
        </div>
        <p v-if="dialogs.current.incorrect" class="rounded-md bg-danger-soft px-2.5 py-1.5 text-xs text-danger">
          {{ dialogs.current.secret === "password" ? "Authentication failed. Please try again." : "Incorrect passphrase. Please try again." }}
        </p>
        <input v-model="text" type="password" class="field" autocomplete="off" autofocus
          :aria-label="dialogs.current.secret === 'password' ? 'Password' : 'Passphrase'" />
        <label v-if="dialogs.current.allowSave" class="flex items-center gap-2 text-xs text-fg-muted">
          <input v-model="save" type="checkbox" class="accent-[var(--accent)]" />
          Save in DevNest vault (encrypted, protected by the OS keychain)
        </label>
      </form>
      <template #footer>
        <button class="btn" @click="cancel">Cancel</button>
        <button class="btn btn-primary" :disabled="!text" @click="accept">Connect</button>
      </template>
    </Modal>

    <!-- Host key verification -->
    <Modal
      v-else-if="dialogs.current.kind === 'hostKey'"
      :title="dialogs.current.changed ? 'Host key has changed!' : 'Unknown host'"
      width="520px"
      @close="cancel"
    >
      <div class="space-y-3">
        <div
          class="flex items-start gap-3 rounded-lg p-3"
          :class="dialogs.current.changed ? 'bg-danger-soft text-danger' : 'bg-accent-soft text-fg'"
        >
          <component :is="dialogs.current.changed ? ShieldAlert : ShieldQuestion" :size="20" class="mt-0.5 shrink-0" />
          <p v-if="dialogs.current.changed" class="text-[13px]">
            The host key for <b>{{ dialogs.current.info.host }}:{{ dialogs.current.info.port }}</b> is different from the one you trusted
            before. Someone could be intercepting the connection (man-in-the-middle), or the server was reinstalled.
            Only continue if you know why the key changed.
          </p>
          <p v-else class="text-[13px]">
            The authenticity of <b>{{ dialogs.current.info.host }}:{{ dialogs.current.info.port }}</b> can't be established. Verify the
            fingerprint with the server administrator before trusting it.
          </p>
        </div>
        <dl class="selectable grid grid-cols-[110px_1fr] gap-x-3 gap-y-1.5 text-xs">
          <dt class="text-fg-muted">Key type</dt>
          <dd class="font-mono">{{ dialogs.current.info.key_type }}</dd>
          <dt class="text-fg-muted">Fingerprint</dt>
          <dd class="font-mono break-all">{{ dialogs.current.info.fingerprint }}</dd>
          <template v-if="dialogs.current.info.previous_fingerprint">
            <dt class="text-fg-muted">Previously</dt>
            <dd class="font-mono break-all text-fg-muted line-through">{{ dialogs.current.info.previous_fingerprint }}</dd>
          </template>
        </dl>
        <label v-if="dialogs.current.changed" class="flex items-center gap-2 text-xs">
          <input v-model="changedAck" type="checkbox" class="accent-[var(--danger)]" />
          I understand the risk and want to replace the stored key
        </label>
      </div>
      <template #footer>
        <button class="btn" @click="cancel">Cancel</button>
        <button
          class="btn"
          :class="dialogs.current.changed ? 'btn-danger' : 'btn-primary'"
          :disabled="dialogs.current.changed && !changedAck"
          @click="accept"
        >
          {{ dialogs.current.changed ? "Replace key & connect" : "Trust & connect" }}
        </button>
      </template>
    </Modal>
  </template>
</template>
