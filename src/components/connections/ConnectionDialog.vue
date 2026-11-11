<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { homeDir, join } from "@tauri-apps/api/path";
import { ChevronDown, ChevronRight, CircleAlert, CircleCheck, FolderOpen, LoaderCircle, PlugZap } from "lucide-vue-next";
import Modal from "@/components/common/Modal.vue";
import { useConnectionsStore } from "@/stores/connections";
import { useToastStore } from "@/stores/toasts";
import { useTabsStore } from "@/stores/tabs";
import { connectionsApi } from "@/modules/connections/api";
import type { Connection, SaveConnectionInput, SecretUpdate } from "@/modules/connections/types";
import { validateConnection, type FormErrors } from "@/modules/connections/validation";
import { CancelledError, runWithPrompts } from "@/modules/ssh/prompts";
import { toAppError } from "@/lib/ipc";

const props = defineProps<{ connection?: Connection | null; defaultGroupId?: string | null }>();
const emit = defineEmits<{ close: []; saved: [c: Connection] }>();

const store = useConnectionsStore();
const toasts = useToastStore();
const tabs = useTabsStore();
const editing = computed(() => !!props.connection);
const c = props.connection;

const form = reactive({
  name: c?.name ?? "",
  groupId: c?.group_id ?? props.defaultGroupId ?? "",
  host: c?.host ?? "",
  port: c?.port ?? 22,
  username: c?.username ?? "",
  authType: c?.auth_type ?? ("password" as "password" | "key"),
  keySource: c?.key_source ?? ("path" as "path" | "vault"),
  keyPath: c?.key_path ?? "",
  password: "",
  clearPassword: false,
  passphrase: "",
  clearPassphrase: false,
  keyMode: (c?.has_private_key ? "keep" : "paste") as "keep" | "paste" | "file",
  keyPaste: "",
  keyImportPath: "",
  sshOptions: c?.ssh_options ?? "",
});
const showAdvanced = ref(!!c?.ssh_options);

const errors = ref<FormErrors>({});
const submitted = ref(false);
const saving = ref(false);
const testing = ref(false);
const testResult = ref<{ ok: boolean; text: string; detail?: string } | null>(null);
const serverError = ref<string | null>(null);

const hasStoredPassword = computed(() => !!c?.has_password && !form.clearPassword);
const hasStoredPassphrase = computed(() => !!c?.has_passphrase && !form.clearPassphrase);

const hasVaultKey = computed(
  () =>
    (form.keyMode === "keep" && !!c?.has_private_key) ||
    (form.keyMode === "paste" && !!form.keyPaste.trim()) ||
    (form.keyMode === "file" && !!form.keyImportPath),
);

function validate() {
  errors.value = validateConnection({ ...form, hasVaultKey: hasVaultKey.value });
  return Object.keys(errors.value).length === 0;
}

function secret(value: string, clear: boolean): SecretUpdate {
  if (value) return { action: "set", value };
  if (clear) return { action: "clear" };
  return { action: "keep" };
}

function buildInput(): SaveConnectionInput {
  const isKey = form.authType === "key";
  let privateKey: SecretUpdate = { action: "keep" };
  if (isKey && form.keySource === "vault") {
    if (form.keyMode === "paste" && form.keyPaste.trim()) privateKey = { action: "set", value: form.keyPaste };
    if (form.keyMode === "file" && form.keyImportPath) privateKey = { action: "import_file", path: form.keyImportPath };
  }
  return {
    group_id: form.groupId || null,
    name: form.name.trim(),
    host: form.host.trim(),
    port: Number(form.port),
    username: form.username.trim(),
    auth_type: form.authType,
    key_source: isKey ? form.keySource : null,
    key_path: isKey && form.keySource === "path" ? form.keyPath.trim() : null,
    ssh_options: form.sshOptions,
    password: isKey ? { action: "keep" } : secret(form.password, form.clearPassword),
    passphrase: isKey ? secret(form.passphrase, form.clearPassphrase) : { action: "keep" },
    private_key: privateKey,
  };
}

async function pickKey(target: "keyPath" | "keyImportPath") {
  let defaultPath: string | undefined;
  try {
    defaultPath = await join(await homeDir(), ".ssh");
  } catch {
    /* ignore */
  }
  const picked = await openFileDialog({ multiple: false, directory: false, defaultPath, title: "Select private key" });
  if (typeof picked === "string") form[target] = picked;
}

async function save() {
  submitted.value = true;
  serverError.value = null;
  if (!validate()) return;
  saving.value = true;
  try {
    const input = buildInput();
    const saved = editing.value ? await store.update(c!.id, input) : await store.create(input);
    tabs.syncTitles(saved);
    toasts.success(editing.value ? "Connection updated" : "Connection created", saved.name);
    emit("saved", saved);
    emit("close");
  } catch (e) {
    serverError.value = toAppError(e).message;
  } finally {
    saving.value = false;
  }
}

async function test() {
  submitted.value = true;
  testResult.value = null;
  if (!validate()) return;
  testing.value = true;
  try {
    const input = buildInput();
    const r = await runWithPrompts(
      { connectionName: form.name || form.host, username: form.username, host: form.host, allowSave: false },
      (ov) => connectionsApi.test(c?.id ?? null, input, ov),
    );
    testResult.value = {
      ok: true,
      text: `Connected successfully in ${r.elapsed_ms} ms`,
      detail: r.host_key ? `${r.host_key.key_type} ${r.host_key.fingerprint}` : undefined,
    };
  } catch (e) {
    if (e instanceof CancelledError) return;
    testResult.value = { ok: false, text: toAppError(e).message };
  } finally {
    testing.value = false;
  }
}

function fieldClass(key: keyof FormErrors) {
  return ["field", submitted.value && errors.value[key] ? "field-error" : ""];
}
</script>

<template>
  <Modal :title="editing ? 'Edit connection' : 'New SSH connection'" width="560px" @close="emit('close')">
    <form class="space-y-4" novalidate @submit.prevent="save" @input="submitted && validate()">
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="label" for="cn-name">Name</label>
          <input id="cn-name" v-model="form.name" :class="fieldClass('name')" placeholder="Production web" autofocus />
          <p v-if="submitted && errors.name" class="mt-1 text-xs text-danger">{{ errors.name }}</p>
        </div>
        <div>
          <label class="label" for="cn-group">Group</label>
          <select id="cn-group" v-model="form.groupId" class="field">
            <option value="">— No group —</option>
            <option v-for="g in store.groups" :key="g.id" :value="g.id">{{ g.name }}</option>
          </select>
        </div>
      </div>

      <div class="grid grid-cols-[1fr_96px] gap-3">
        <div>
          <label class="label" for="cn-host">Hostname / IP</label>
          <input id="cn-host" v-model="form.host" :class="fieldClass('host')" placeholder="203.0.113.10" spellcheck="false" />
          <p v-if="submitted && errors.host" class="mt-1 text-xs text-danger">{{ errors.host }}</p>
        </div>
        <div>
          <label class="label" for="cn-port">Port</label>
          <input id="cn-port" v-model.number="form.port" type="number" min="1" max="65535" :class="fieldClass('port')" />
          <p v-if="submitted && errors.port" class="mt-1 text-xs text-danger">{{ errors.port }}</p>
        </div>
      </div>

      <div>
        <label class="label" for="cn-user">Username</label>
        <input id="cn-user" v-model="form.username" :class="fieldClass('username')" placeholder="ubuntu" spellcheck="false" autocomplete="off" />
        <p v-if="submitted && errors.username" class="mt-1 text-xs text-danger">{{ errors.username }}</p>
      </div>

      <fieldset>
        <legend class="label">Authentication</legend>
        <div class="inline-flex rounded-md border border-line p-0.5">
          <button
            v-for="opt in [{ v: 'password', l: 'Password' }, { v: 'key', l: 'Private key' }] as const"
            :key="opt.v"
            type="button"
            class="rounded px-3 py-1 text-xs font-medium"
            :class="form.authType === opt.v ? 'bg-accent text-accent-fg' : 'text-fg-muted hover:text-fg'"
            :aria-pressed="form.authType === opt.v"
            @click="form.authType = opt.v"
          >
            {{ opt.l }}
          </button>
        </div>
      </fieldset>

      <!-- Password -->
      <div v-if="form.authType === 'password'">
        <label class="label" for="cn-pw">Password</label>
        <input
          id="cn-pw"
          v-model="form.password"
          type="password"
          class="field"
          autocomplete="new-password"
          :placeholder="hasStoredPassword ? '•••••••• (saved – leave empty to keep)' : 'Leave empty to be asked on connect'"
        />
        <label v-if="c?.has_password" class="mt-1.5 flex items-center gap-2 text-xs text-fg-muted">
          <input v-model="form.clearPassword" type="checkbox" class="accent-[var(--accent)]" /> Remove saved password
        </label>
      </div>

      <!-- Private key -->
      <div v-else class="space-y-3 rounded-lg border border-line bg-surface-2 p-3">
        <div class="flex gap-4 text-xs">
          <label class="flex items-center gap-1.5">
            <input v-model="form.keySource" type="radio" value="path" class="accent-[var(--accent)]" /> Use key file on this computer
          </label>
          <label class="flex items-center gap-1.5">
            <input v-model="form.keySource" type="radio" value="vault" class="accent-[var(--accent)]" /> Store key in DevNest vault
          </label>
        </div>

        <div v-if="form.keySource === 'path'">
          <label class="label" for="cn-keypath">Private key file</label>
          <div class="flex gap-2">
            <input id="cn-keypath" v-model="form.keyPath" :class="fieldClass('keyPath')" placeholder="~/.ssh/id_ed25519" spellcheck="false" />
            <button type="button" class="btn shrink-0" @click="pickKey('keyPath')"><FolderOpen :size="14" /> Browse</button>
          </div>
          <p v-if="submitted && errors.keyPath" class="mt-1 text-xs text-danger">{{ errors.keyPath }}</p>
          <p class="mt-1 text-xs text-fg-subtle">OpenSSH, PEM (RSA/ECDSA/Ed25519) and PuTTY .ppk keys are supported.</p>
        </div>

        <div v-else class="space-y-2">
          <div class="flex gap-3 text-xs">
            <label v-if="c?.has_private_key" class="flex items-center gap-1.5">
              <input v-model="form.keyMode" type="radio" value="keep" class="accent-[var(--accent)]" /> Keep stored key
            </label>
            <label class="flex items-center gap-1.5">
              <input v-model="form.keyMode" type="radio" value="paste" class="accent-[var(--accent)]" /> Paste key
            </label>
            <label class="flex items-center gap-1.5">
              <input v-model="form.keyMode" type="radio" value="file" class="accent-[var(--accent)]" /> Import from file
            </label>
          </div>
          <textarea
            v-if="form.keyMode === 'paste'"
            v-model="form.keyPaste"
            rows="5"
            spellcheck="false"
            aria-label="Private key"
            class="field h-auto py-2 font-mono text-[11px] leading-snug"
            :class="submitted && errors.privateKey ? 'field-error' : ''"
            placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
          />
          <div v-else-if="form.keyMode === 'file'" class="flex gap-2">
            <input v-model="form.keyImportPath" class="field" placeholder="Select a key file to import" readonly aria-label="Key file to import" />
            <button type="button" class="btn shrink-0" @click="pickKey('keyImportPath')"><FolderOpen :size="14" /> Browse</button>
          </div>
          <p v-if="submitted && errors.privateKey" class="text-xs text-danger">{{ errors.privateKey }}</p>
          <p class="text-xs text-fg-subtle">The key is encrypted and never leaves the backend. Stored keys can later be exported to another user.</p>
        </div>

        <div>
          <label class="label" for="cn-pp">Passphrase <span class="font-normal text-fg-subtle">(if the key is encrypted)</span></label>
          <input
            id="cn-pp"
            v-model="form.passphrase"
            type="password"
            class="field"
            autocomplete="new-password"
            :placeholder="hasStoredPassphrase ? '•••••••• (saved – leave empty to keep)' : 'Leave empty to be asked on connect'"
          />
          <label v-if="c?.has_passphrase" class="mt-1.5 flex items-center gap-2 text-xs text-fg-muted">
            <input v-model="form.clearPassphrase" type="checkbox" class="accent-[var(--accent)]" /> Remove saved passphrase
          </label>
        </div>
      </div>

      <!-- Advanced: legacy algorithm options -->
      <div>
        <button type="button" class="flex items-center gap-1 text-xs font-medium text-fg-muted hover:text-fg" @click="showAdvanced = !showAdvanced">
          <component :is="showAdvanced ? ChevronDown : ChevronRight" :size="14" /> Advanced
          <span v-if="form.sshOptions.trim() && !showAdvanced" class="ml-1 rounded bg-accent-soft px-1 text-[10px] text-accent">SSH options</span>
        </button>
        <div v-if="showAdvanced" class="mt-2">
          <label class="label" for="cn-opts">SSH options <span class="font-normal text-fg-subtle">(like <code>ssh -o</code>, one per line)</span></label>
          <textarea
            id="cn-opts"
            v-model="form.sshOptions"
            rows="3"
            spellcheck="false"
            class="field h-auto py-2 font-mono text-[11px] leading-snug"
            placeholder="HostKeyAlgorithms=+ssh-rsa&#10;PubkeyAcceptedKeyTypes=+ssh-rsa"
          />
          <p class="mt-1 text-xs text-fg-subtle">
            Supported: HostKeyAlgorithms, PubkeyAcceptedKeyTypes, KexAlgorithms, Ciphers, MACs. Use <code>+</code> to add legacy
            algorithms for old servers.
          </p>
        </div>
      </div>

      <div
        v-if="testResult"
        class="flex items-start gap-2 rounded-md px-3 py-2 text-xs"
        :class="testResult.ok ? 'bg-accent-soft text-fg' : 'bg-danger-soft text-danger'"
        role="status"
      >
        <component :is="testResult.ok ? CircleCheck : CircleAlert" :size="14" class="mt-0.5 shrink-0" :class="testResult.ok ? 'text-ok' : ''" />
        <div class="selectable min-w-0">
          <p>{{ testResult.text }}</p>
          <p v-if="testResult.detail" class="mt-0.5 font-mono break-all text-fg-muted">{{ testResult.detail }}</p>
        </div>
      </div>
      <p v-if="serverError" class="selectable rounded-md bg-danger-soft px-3 py-2 text-xs text-danger" role="alert">{{ serverError }}</p>
      <button type="submit" class="hidden" />
    </form>

    <template #footer>
      <button class="btn mr-auto" :disabled="testing || saving" @click="test">
        <LoaderCircle v-if="testing" :size="14" class="animate-spin" />
        <PlugZap v-else :size="14" />
        Test connection
      </button>
      <button class="btn" @click="emit('close')">Cancel</button>
      <button class="btn btn-primary" :disabled="saving" @click="save">
        <LoaderCircle v-if="saving" :size="14" class="animate-spin" />
        {{ editing ? "Save" : "Create" }}
      </button>
    </template>
  </Modal>
</template>
