import { defineStore } from "pinia";
import { shallowRef } from "vue";
import type { HostKeyInfo } from "@/modules/connections/types";

/** Promise-based modal dialogs rendered by <DialogHost>. One at a time. */
export type DialogRequest =
  | {
      kind: "confirm";
      title: string;
      message: string;
      confirmText?: string;
      danger?: boolean;
      resolve: (ok: boolean) => void;
    }
  | {
      kind: "input";
      title: string;
      label: string;
      value: string;
      placeholder?: string;
      confirmText?: string;
      resolve: (value: string | null) => void;
    }
  | {
      kind: "credential";
      secret: "password" | "passphrase";
      connectionName: string;
      username?: string;
      host?: string;
      incorrect?: boolean;
      allowSave: boolean;
      resolve: (r: { value: string; save: boolean } | null) => void;
    }
  | {
      kind: "hostKey";
      info: HostKeyInfo;
      changed: boolean;
      resolve: (trust: boolean) => void;
    };

type Without<T> = T extends unknown ? Omit<T, "resolve" | "kind"> : never;

export const useDialogStore = defineStore("dialogs", () => {
  const current = shallowRef<DialogRequest | null>(null);
  const queue: DialogRequest[] = [];

  function open(req: DialogRequest) {
    if (current.value) queue.push(req);
    else current.value = req;
  }

  function close() {
    current.value = queue.shift() ?? null;
  }

  function confirm(opts: Without<Extract<DialogRequest, { kind: "confirm" }>>): Promise<boolean> {
    return new Promise((resolve) => open({ kind: "confirm", ...opts, resolve }));
  }

  function input(opts: Without<Extract<DialogRequest, { kind: "input" }>>): Promise<string | null> {
    return new Promise((resolve) => open({ kind: "input", ...opts, resolve }));
  }

  function credential(
    opts: Without<Extract<DialogRequest, { kind: "credential" }>>,
  ): Promise<{ value: string; save: boolean } | null> {
    return new Promise((resolve) => open({ kind: "credential", ...opts, resolve }));
  }

  function hostKey(info: HostKeyInfo, changed: boolean): Promise<boolean> {
    return new Promise((resolve) => open({ kind: "hostKey", info, changed, resolve }));
  }

  return { current, close, confirm, input, credential, hostKey };
});
