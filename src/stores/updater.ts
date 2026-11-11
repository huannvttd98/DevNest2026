import { defineStore } from "pinia";
import { computed, ref, shallowRef } from "vue";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { useDialogStore } from "@/stores/dialogs";
import { useTabsStore } from "@/stores/tabs";
import { useToastStore } from "@/stores/toasts";
import { errorMessage } from "@/lib/ipc";

export type UpdaterStatus = "idle" | "checking" | "available" | "downloading" | "installing" | "error";

const AUTO_CHECK_DELAY_MS = 5_000;
const AUTO_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;
const DISMISSED_KEY = "devnest.update.dismissed";

function readDismissed(): string | null {
  try {
    return localStorage.getItem(DISMISSED_KEY);
  } catch {
    return null;
  }
}

/**
 * In-app updates (tauri-plugin-updater + GitHub Releases `latest.json`).
 * The download is verified against the public key in tauri.conf.json before install.
 */
export const useUpdaterStore = defineStore("updater", () => {
  const status = ref<UpdaterStatus>("idle");
  const update = shallowRef<Update | null>(null);
  const error = ref<string | null>(null);
  const downloaded = ref(0);
  const total = ref<number | null>(null);
  const dialogOpen = ref(false);
  const lastChecked = ref<Date | null>(null);
  let timer: number | undefined;

  const progress = computed(() => (total.value ? Math.min(100, Math.round((downloaded.value / total.value) * 100)) : null));
  const busy = computed(() => status.value === "downloading" || status.value === "installing");

  /** Check for a newer version. `manual` = user clicked "Check for updates". */
  async function checkForUpdates(manual = false) {
    if (status.value === "checking" || busy.value) return;
    const toasts = useToastStore();
    status.value = "checking";
    error.value = null;
    try {
      const found = await check({ timeout: 30_000 });
      lastChecked.value = new Date();
      if (found) {
        await update.value?.close().catch(() => {});
        update.value = found;
        status.value = "available";
        // Pop the dialog unless the user already postponed this exact version.
        if (manual || readDismissed() !== found.version) dialogOpen.value = true;
      } else {
        status.value = "idle";
        if (manual) toasts.success("DevNest is up to date", "You are running the latest version.");
      }
    } catch (e) {
      status.value = update.value ? "available" : "idle";
      error.value = errorMessage(e);
      // Automatic checks fail silently (offline, no release yet...).
      if (manual) toasts.error("Could not check for updates", error.value);
    }
  }

  function later() {
    if (update.value) {
      try {
        localStorage.setItem(DISMISSED_KEY, update.value.version);
      } catch {
        /* ignore */
      }
    }
    dialogOpen.value = false;
  }

  async function install() {
    const u = update.value;
    if (!u || busy.value) return;
    const dialogs = useDialogStore();
    const tabs = useTabsStore();

    const open = tabs.tabs.filter((t) => t.state === "connected" || t.state === "connecting").length;
    if (open > 0) {
      const ok = await dialogs.confirm({
        title: "Install update",
        message: `DevNest will restart to finish the update.\n${open} open terminal session(s) will be disconnected.`,
        confirmText: "Update & restart",
      });
      if (!ok) return;
    }

    status.value = "downloading";
    downloaded.value = 0;
    total.value = null;
    error.value = null;
    try {
      await u.downloadAndInstall((ev) => {
        if (ev.event === "Started") total.value = ev.data.contentLength ?? null;
        else if (ev.event === "Progress") downloaded.value += ev.data.chunkLength;
        else if (ev.event === "Finished") status.value = "installing";
      });
      // On Windows the installer usually closes the app itself; elsewhere restart now.
      await relaunch();
    } catch (e) {
      status.value = "error";
      error.value = errorMessage(e);
    }
  }

  function start() {
    if (import.meta.env.DEV) return; // dev builds: manual checks only
    window.setTimeout(() => void checkForUpdates(false), AUTO_CHECK_DELAY_MS);
    clearInterval(timer);
    timer = window.setInterval(() => void checkForUpdates(false), AUTO_CHECK_INTERVAL_MS);
  }

  return {
    status,
    update,
    error,
    downloaded,
    total,
    progress,
    busy,
    dialogOpen,
    lastChecked,
    checkForUpdates,
    later,
    install,
    start,
  };
});
