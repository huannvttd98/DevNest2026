import { describe, expect, it } from "vitest";
import { validateConnection, type ConnectionFormValues } from "./validation";

const base: ConnectionFormValues = {
  name: "web-01",
  host: "192.168.1.10",
  port: 22,
  username: "deploy",
  authType: "password",
  keySource: "path",
  keyPath: "",
  hasVaultKey: false,
};

describe("validateConnection", () => {
  it("accepts a valid password connection", () => {
    expect(validateConnection(base)).toEqual({});
  });

  it("requires name, host and username", () => {
    const e = validateConnection({ ...base, name: " ", host: "", username: "" });
    expect(Object.keys(e).sort()).toEqual(["host", "name", "username"]);
  });

  it("validates host format", () => {
    expect(validateConnection({ ...base, host: "bad host!" }).host).toBeDefined();
    expect(validateConnection({ ...base, host: "my-server.example.com" }).host).toBeUndefined();
    expect(validateConnection({ ...base, host: "::1" }).host).toBeUndefined();
    expect(validateConnection({ ...base, host: "fe80::1" }).host).toBeUndefined();
  });

  it("validates port range", () => {
    expect(validateConnection({ ...base, port: 0 }).port).toBeDefined();
    expect(validateConnection({ ...base, port: 70000 }).port).toBeDefined();
    expect(validateConnection({ ...base, port: "2222" }).port).toBeUndefined();
    expect(validateConnection({ ...base, port: "abc" }).port).toBeDefined();
  });

  it("rejects usernames with spaces", () => {
    expect(validateConnection({ ...base, username: "a b" }).username).toBeDefined();
  });

  it("requires key path or vault key for key auth", () => {
    expect(validateConnection({ ...base, authType: "key" }).keyPath).toBeDefined();
    expect(validateConnection({ ...base, authType: "key", keyPath: "~/.ssh/id_ed25519" })).toEqual({});
    expect(validateConnection({ ...base, authType: "key", keySource: "vault" }).privateKey).toBeDefined();
    expect(validateConnection({ ...base, authType: "key", keySource: "vault", hasVaultKey: true })).toEqual({});
  });
});
