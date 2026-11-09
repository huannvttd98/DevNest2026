import { connectionsApi } from "@/modules/connections/api";
import type { CredentialOverrides, HostKeyInfo } from "@/modules/connections/types";
import { toAppError, type AppError } from "@/lib/ipc";
import { useDialogStore } from "@/stores/dialogs";

export class CancelledError extends Error {
  constructor() {
    super("Cancelled");
  }
}

export interface PromptContext {
  connectionName: string;
  username?: string;
  host?: string;
  /** Offer "save to vault" in credential prompts. */
  allowSave: boolean;
}

/**
 * Run an SSH operation and resolve interactive requirements reported by the backend:
 * unknown / changed host key, missing password, missing or wrong key passphrase.
 * Retries with the user's answers; throws CancelledError if the user cancels.
 */
export async function runWithPrompts<T>(
  ctx: PromptContext,
  op: (overrides: CredentialOverrides, save: boolean) => Promise<T>,
): Promise<T> {
  const dialogs = useDialogStore();
  const overrides: CredentialOverrides = {};
  let save = false;

  for (let attempt = 0; attempt < 6; attempt++) {
    try {
      return await op(overrides, save);
    } catch (raw) {
      const e: AppError = toAppError(raw);
      switch (e.code) {
        case "HOST_KEY_UNKNOWN":
        case "HOST_KEY_CHANGED": {
          const info = e.details as unknown as HostKeyInfo;
          const changed = e.code === "HOST_KEY_CHANGED";
          if (!(await dialogs.hostKey(info, changed))) throw new CancelledError();
          await connectionsApi.trustHostKey(info, changed);
          continue;
        }
        case "AUTH_PASSWORD_REQUIRED":
        case "SSH_AUTH_FAILED": {
          // A failed password login: let the user retype it (only for password logins we prompted).
          if (e.code === "SSH_AUTH_FAILED" && overrides.password === undefined) throw e;
          const r = await dialogs.credential({
            secret: "password",
            connectionName: ctx.connectionName,
            username: ctx.username,
            host: ctx.host,
            incorrect: e.code === "SSH_AUTH_FAILED",
            allowSave: ctx.allowSave,
          });
          if (!r) throw new CancelledError();
          overrides.password = r.value;
          save = save || r.save;
          continue;
        }
        case "KEY_PASSPHRASE_REQUIRED": {
          const r = await dialogs.credential({
            secret: "passphrase",
            connectionName: ctx.connectionName,
            incorrect: e.details?.incorrect === true,
            allowSave: ctx.allowSave,
          });
          if (!r) throw new CancelledError();
          overrides.passphrase = r.value;
          save = save || r.save;
          continue;
        }
        default:
          throw e;
      }
    }
  }
  throw toAppError(new Error("Too many attempts"));
}
