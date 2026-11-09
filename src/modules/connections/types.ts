export type AuthType = "password" | "key";
export type KeySource = "path" | "vault";

export interface Group {
  id: string;
  name: string;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

/** Saved connection as returned by the backend. Never contains secret values. */
export interface Connection {
  id: string;
  group_id: string | null;
  name: string;
  host: string;
  port: number;
  username: string;
  auth_type: AuthType;
  key_source: KeySource | null;
  key_path: string | null;
  has_password: boolean;
  has_passphrase: boolean;
  has_private_key: boolean;
  sort_order: number;
  created_at: string;
  updated_at: string;
  last_connected_at: string | null;
}

export type SecretUpdate =
  | { action: "keep" }
  | { action: "set"; value: string }
  | { action: "clear" }
  | { action: "import_file"; path: string };

export interface SaveConnectionInput {
  group_id: string | null;
  name: string;
  host: string;
  port: number;
  username: string;
  auth_type: AuthType;
  key_source: KeySource | null;
  key_path: string | null;
  password: SecretUpdate;
  passphrase: SecretUpdate;
  private_key: SecretUpdate;
}

export interface CredentialOverrides {
  password?: string;
  passphrase?: string;
}

export interface HostKeyInfo {
  host: string;
  port: number;
  key_type: string;
  fingerprint: string;
  previous_fingerprint?: string;
}
