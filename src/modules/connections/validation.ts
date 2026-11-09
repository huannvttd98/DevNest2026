/** Client-side validation mirroring the backend rules (backend stays authoritative). */
export interface ConnectionFormValues {
  name: string;
  host: string;
  port: number | string;
  username: string;
  authType: "password" | "key";
  keySource: "path" | "vault";
  keyPath: string;
  /** vault mode: true when a key is already stored or a new one is provided */
  hasVaultKey: boolean;
}

export type FormErrors = Partial<Record<"name" | "host" | "port" | "username" | "keyPath" | "privateKey", string>>;

const HOST_RE = /^[A-Za-z0-9._-]+$/;
const IPV6_RE = /^\[?[0-9A-Fa-f:.]+(%\w+)?\]?$/;

export function validateConnection(v: ConnectionFormValues): FormErrors {
  const e: FormErrors = {};
  const name = v.name.trim();
  const host = v.host.trim();
  const user = v.username.trim();
  const port = typeof v.port === "string" ? Number(v.port) : v.port;

  if (!name) e.name = "Name is required";
  else if ([...name].length > 100) e.name = "At most 100 characters";

  if (!host) e.host = "Hostname / IP is required";
  else if (host.length > 253) e.host = "Hostname is too long";
  else if (!HOST_RE.test(host) && !(host.includes(":") && IPV6_RE.test(host))) e.host = "Invalid hostname or IP";

  if (!Number.isInteger(port) || port < 1 || port > 65535) e.port = "1 – 65535";

  if (!user) e.username = "Username is required";
  else if (/\s/.test(user)) e.username = "No spaces allowed";

  if (v.authType === "key") {
    if (v.keySource === "path" && !v.keyPath.trim()) e.keyPath = "Choose a private key file";
    if (v.keySource === "vault" && !v.hasVaultKey) e.privateKey = "Paste or import a private key";
  }
  return e;
}
