<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowUp, File, Folder, HardDrive, LoaderCircle, RefreshCw, Server } from "lucide-vue-next";
import DialogHost from "@/components/dialogs/DialogHost.vue";
import Toasts from "@/components/common/Toasts.vue";
import { connectionsApi } from "@/modules/connections/api";
import { filesApi, type LocalDirectory, type LocalFileEntry } from "@/modules/files/api";
import { sshApi, type RemoteDirectory, type RemoteFileEntry } from "@/modules/ssh/api";
import { toAppError } from "@/lib/ipc";
import { useDialogStore } from "@/stores/dialogs";
import { useToastStore } from "@/stores/toasts";
import { useUiStore } from "@/stores/ui";
import type { Connection } from "@/modules/connections/types";

const props = defineProps<{ connectionId: string }>();

const dialogs = useDialogStore();
const toasts = useToastStore();
useUiStore();

const connection = ref<Connection | null>(null);
const remoteDirectory = ref<RemoteDirectory | null>(null);
const remoteLoading = ref(false);
const remoteError = ref("");
const localDirectory = ref<LocalDirectory | null>(null);
const localRoot = ref<string | null>(null);
const localLoading = ref(false);
const localError = ref("");
const dragOver = ref(false);
const transferring = ref<string | null>(null);

const remoteParent = computed(() => (remoteDirectory.value ? parentRemotePath(remoteDirectory.value.path) : null));
const isAtLocalRoot = computed(() => localDirectory.value?.path === localRoot.value);

function parentRemotePath(path: string): string | null {
  const trimmed = path.replace(/\/+$/, "");
  if (!trimmed || trimmed === "/") return null;
  const separator = trimmed.lastIndexOf("/");
  return separator <= 0 ? "/" : trimmed.slice(0, separator);
}

function formatSize(size: number | null): string {
  if (size === null) return "";
  if (size < 1024) return `${size} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = size / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
}

async function loadRemote(path = remoteDirectory.value?.path ?? ".") {
  if (!connection.value) return;
  remoteLoading.value = true;
  remoteError.value = "";
  try {
    remoteDirectory.value = await sshApi.listRemoteDir(connection.value.id, path);
  } catch (error) {
    remoteError.value = toAppError(error).message;
  } finally {
    remoteLoading.value = false;
  }
}

async function loadLocal(path: string): Promise<LocalDirectory | null> {
  localLoading.value = true;
  localError.value = "";
  try {
    const directory = await filesApi.listLocalDir(path);
    localDirectory.value = directory;
    return directory;
  } catch (error) {
    localError.value = toAppError(error).message;
    return null;
  } finally {
    localLoading.value = false;
  }
}

async function chooseLocalFolder() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Choose a local download folder",
    });
    if (typeof selected !== "string") return;
    localRoot.value = null;
    localDirectory.value = null;
    const directory = await loadLocal(selected);
    if (directory) localRoot.value = directory.path;
  } catch (error) {
    toasts.error("Cannot open local folder", toAppError(error).message);
  }
}

function goUpRemote() {
  if (remoteParent.value) void loadRemote(remoteParent.value);
}

function goUpLocal() {
  const parent = localDirectory.value?.parent;
  if (parent && !isAtLocalRoot.value) void loadLocal(parent);
}

function startDrag(event: DragEvent, entry: RemoteFileEntry) {
  if (!event.dataTransfer || entry.is_dir) return;
  event.dataTransfer.effectAllowed = "copy";
  event.dataTransfer.setData(
    "application/x-devnest-remote-file",
    JSON.stringify({ path: entry.path, name: entry.name }),
  );
  event.dataTransfer.setData("text/plain", entry.name);
}

function allowDirectoryDrop(event: DragEvent) {
  event.preventDefault();
  dragOver.value = true;
}

function droppedFile(event: DragEvent): { path: string; name: string } | null {
  const raw = event.dataTransfer?.getData("application/x-devnest-remote-file");
  if (!raw) return null;
  try {
    const value: unknown = JSON.parse(raw);
    if (
      typeof value === "object" &&
      value !== null &&
      "path" in value &&
      "name" in value &&
      typeof value.path === "string" &&
      typeof value.name === "string"
    ) {
      return { path: value.path, name: value.name };
    }
  } catch {
    toasts.error("Download failed", "The dragged file information is invalid.");
  }
  return null;
}

async function dropFile(event: DragEvent, target: LocalFileEntry | LocalDirectory) {
  event.preventDefault();
  event.stopPropagation();
  dragOver.value = false;
  const file = droppedFile(event);
  if (!file || !connection.value || transferring.value) return;

  transferring.value = file.path;
  try {
    let savedPath: string;
    try {
      savedPath = await sshApi.download(connection.value.id, file.path, target.path, file.name);
    } catch (error) {
      if (toAppError(error).code !== "FILE_EXISTS") throw error;
      const replace = await dialogs.confirm({
        title: "Replace local file?",
        message: `${file.name} already exists in this folder. Do you want to replace it?`,
        confirmText: "Replace",
        danger: true,
      });
      if (!replace) return;
      savedPath = await sshApi.download(connection.value.id, file.path, target.path, file.name, true);
    }
    toasts.success("File downloaded", savedPath);
    await loadLocal(target.path);
  } catch (error) {
    toasts.error("Download failed", toAppError(error).message);
  } finally {
    transferring.value = null;
  }
}

onMounted(async () => {
  if (!props.connectionId) {
    remoteError.value = "No SSH connection was specified.";
    return;
  }
  try {
    const all = await connectionsApi.list();
    connection.value = all.find((item) => item.id === props.connectionId) ?? null;
    if (!connection.value) {
      remoteError.value = "This SSH connection no longer exists.";
      return;
    }
    await loadRemote(".");
  } catch (error) {
    remoteError.value = toAppError(error).message;
  }
});
</script>

<template>
  <main class="flex h-full flex-col bg-app">
    <header class="flex h-12 shrink-0 items-center gap-2 border-b border-line bg-surface px-4">
      <Server :size="16" class="text-accent" />
      <div class="min-w-0 flex-1">
        <h1 class="truncate text-sm font-semibold">Server files</h1>
        <p class="truncate text-[11px] text-fg-muted">
          {{
            connection ? `${connection.username}@${connection.host}:${connection.port}` : "Connecting to file browser…"
          }}
        </p>
      </div>
      <span class="hidden text-xs text-fg-subtle sm:inline">Drag a server file to a local folder to download</span>
    </header>

    <div class="grid min-h-0 flex-1 grid-cols-2 divide-x divide-line">
      <section class="flex min-w-0 flex-col">
        <div class="flex h-10 shrink-0 items-center gap-2 border-b border-line bg-surface-2 px-3">
          <button
            class="icon-btn"
            :disabled="!remoteParent || remoteLoading"
            title="Go to parent folder"
            aria-label="Go to parent folder"
            @click="goUpRemote"
          >
            <ArrowUp :size="15" />
          </button>
          <div class="min-w-0 flex-1 truncate font-mono text-xs" :title="remoteDirectory?.path">
            {{ remoteDirectory?.path ?? "Server" }}
          </div>
          <button
            class="icon-btn"
            :disabled="remoteLoading || !connection"
            title="Refresh server files"
            @click="loadRemote()"
          >
            <RefreshCw :size="14" :class="remoteLoading ? 'animate-spin' : ''" />
          </button>
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto p-1">
          <div
            v-if="remoteLoading && !remoteDirectory"
            class="flex h-full items-center justify-center gap-2 text-xs text-fg-muted"
          >
            <LoaderCircle :size="15" class="animate-spin" /> Loading server files…
          </div>
          <div v-else-if="remoteError" class="m-3 rounded-md bg-danger-soft p-3 text-xs text-danger" role="alert">
            <p>{{ remoteError }}</p>
            <button class="btn mt-2 h-7 text-xs" @click="loadRemote()">Retry</button>
          </div>
          <div v-else-if="!remoteDirectory?.entries.length" class="p-4 text-center text-xs text-fg-muted">
            This folder is empty.
          </div>
          <button
            v-for="entry in remoteDirectory?.entries"
            :key="entry.path"
            class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs hover:bg-hover"
            :class="!entry.is_dir ? 'cursor-grab active:cursor-grabbing' : 'cursor-default'"
            :draggable="!entry.is_dir"
            :title="entry.is_dir ? 'Double-click to open folder' : 'Drag this file to a local folder'"
            @dblclick="entry.is_dir && loadRemote(entry.path)"
            @dragstart="startDrag($event, entry)"
          >
            <Folder v-if="entry.is_dir" :size="15" class="shrink-0 text-accent" />
            <File v-else :size="15" class="shrink-0 text-fg-subtle" />
            <span class="min-w-0 flex-1 truncate">{{ entry.name }}</span>
            <span v-if="!entry.is_dir" class="shrink-0 text-[10px] text-fg-subtle">{{ formatSize(entry.size) }}</span>
            <LoaderCircle v-if="transferring === entry.path" :size="13" class="animate-spin text-accent" />
          </button>
        </div>
      </section>

      <section
        class="flex min-w-0 flex-col transition-colors"
        :class="dragOver ? 'bg-accent-soft/50' : 'bg-surface'"
        @dragover.prevent="dragOver = true"
        @dragleave="dragOver = false"
        @drop="localDirectory && dropFile($event, localDirectory)"
      >
        <div class="flex h-10 shrink-0 items-center gap-2 border-b border-line bg-surface-2 px-3">
          <button
            class="icon-btn"
            :disabled="!localDirectory || isAtLocalRoot || localLoading"
            title="Go to parent folder"
            aria-label="Go to parent folder"
            @click="goUpLocal"
          >
            <ArrowUp :size="15" />
          </button>
          <div class="min-w-0 flex-1 truncate font-mono text-xs" :title="localDirectory?.path">
            {{ localDirectory?.path ?? "Choose a local folder" }}
          </div>
          <button
            v-if="localDirectory"
            class="icon-btn"
            :disabled="localLoading"
            title="Refresh local folder"
            @click="loadLocal(localDirectory.path)"
          >
            <RefreshCw :size="14" :class="localLoading ? 'animate-spin' : ''" />
          </button>
          <button
            class="icon-btn"
            title="Choose local folder"
            aria-label="Choose local folder"
            @click="chooseLocalFolder"
          >
            <HardDrive :size="15" />
          </button>
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto p-1">
          <div
            v-if="localLoading && !localDirectory"
            class="flex h-full items-center justify-center gap-2 text-xs text-fg-muted"
          >
            <LoaderCircle :size="15" class="animate-spin" /> Loading local folder…
          </div>
          <div v-else-if="localError" class="m-3 rounded-md bg-danger-soft p-3 text-xs text-danger" role="alert">
            <p>{{ localError }}</p>
            <button v-if="localDirectory" class="btn mt-2 h-7 text-xs" @click="loadLocal(localDirectory.path)">
              Retry
            </button>
            <button v-else class="btn mt-2 h-7 text-xs" @click="chooseLocalFolder">Choose another folder</button>
          </div>
          <div
            v-else-if="!localDirectory"
            class="flex h-full flex-col items-center justify-center gap-2 px-6 text-center text-xs text-fg-muted"
          >
            <HardDrive :size="24" class="text-fg-subtle" />
            <p>Choose a local folder, then drop server files here or onto a folder.</p>
            <button class="btn btn-primary h-7 text-xs" @click="chooseLocalFolder">Choose folder</button>
          </div>
          <div
            v-else-if="!localDirectory.entries.length"
            class="flex h-full items-center justify-center text-xs text-fg-muted"
          >
            Drop files here to download them into this folder.
          </div>
          <button
            v-for="entry in localDirectory?.entries"
            :key="entry.path"
            class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs"
            :class="entry.is_dir ? 'hover:bg-hover' : 'text-fg-muted'"
            :title="entry.is_dir ? 'Double-click to open folder or drop a server file here' : entry.name"
            @dblclick="entry.is_dir && loadLocal(entry.path)"
            @dragover.stop="entry.is_dir && allowDirectoryDrop($event)"
            @dragleave.stop="dragOver = false"
            @drop.stop.prevent="entry.is_dir && dropFile($event, entry)"
          >
            <Folder v-if="entry.is_dir" :size="15" class="shrink-0 text-accent" />
            <File v-else :size="15" class="shrink-0 text-fg-subtle" />
            <span class="min-w-0 flex-1 truncate">{{ entry.name }}</span>
            <span v-if="!entry.is_dir" class="shrink-0 text-[10px] text-fg-subtle">{{ formatSize(entry.size) }}</span>
          </button>
        </div>
      </section>
    </div>

    <DialogHost />
    <Toasts />
  </main>
</template>
