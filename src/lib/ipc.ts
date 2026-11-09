import { invoke } from "@tauri-apps/api/core";

/** Error codes produced by the Rust backend (`error::ErrorCode`). */
export type ErrorCode =
  | "INTERNAL"
  | "DATABASE"
  | "VAULT"
  | "NOT_FOUND"
  | "VALIDATION"
  | "SSH_CONNECT"
  | "SSH_TIMEOUT"
  | "SSH_AUTH_FAILED"
  | "AUTH_PASSWORD_REQUIRED"
  | "KEY_PASSPHRASE_REQUIRED"
  | "KEY_INVALID"
  | "KEY_FILE_NOT_FOUND"
  | "HOST_KEY_UNKNOWN"
  | "HOST_KEY_CHANGED"
  | "SSH_DISCONNECTED"
  | "SSH_CHANNEL";

export interface AppError {
  code: ErrorCode;
  message: string;
  details?: Record<string, unknown>;
}

export function isAppError(e: unknown): e is AppError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

/** Normalize anything thrown by `invoke` into an AppError. */
export function toAppError(e: unknown): AppError {
  if (isAppError(e)) return e;
  if (e instanceof Error) return { code: "INTERNAL", message: e.message };
  return { code: "INTERNAL", message: String(e) };
}

/** Typed wrapper around `invoke` that always rejects with an AppError. */
export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toAppError(e);
  }
}

export function errorMessage(e: unknown): string {
  return toAppError(e).message;
}
