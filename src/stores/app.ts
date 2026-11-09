import { defineStore } from "pinia";
import { ref } from "vue";
import { call } from "@/lib/ipc";

export interface AppInfo {
  version: string;
  data_dir: string;
  log_dir: string;
  vault_state: "ready" | "created" | null;
  vault_error: string | null;
}

export const useAppStore = defineStore("app", () => {
  const info = ref<AppInfo | null>(null);

  async function load() {
    try {
      info.value = await call<AppInfo>("app_info");
    } catch {
      info.value = null;
    }
    return info.value;
  }

  return { info, load };
});
