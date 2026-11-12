# DevNest

All-in-one Desktop Workspace for Developers — SSH connection manager + terminal (Phase 1–2).
Built with Tauri 2 · Vue 3 · TypeScript · Rust · Tailwind CSS · xterm.js · russh · SQLite.

- Requirements: [devnest-requirements.md](devnest-requirements.md)
- Plan & progress: [docs/PLAN.md](docs/PLAN.md)

## Features (current)

- **SSH Connection Manager** — create / edit / delete connections, groups, filter, move between groups.
- **Authentication** — password or private key (OpenSSH, PEM, PuTTY `.ppk`; Ed25519 / ECDSA / RSA),
  key from a file path or stored in the DevNest vault, optional passphrase.
- **Test connection** from the form, with clear error messages (unreachable, timeout, auth failed, bad key…).
- **Host key verification (TOFU)** — unknown keys must be confirmed; a changed key shows a MITM warning.
- **Prompts on demand** — missing password / passphrase is asked at connect time, optionally saved.
- **SSH Terminal** — xterm.js, multiple tabs (rename, close, Ctrl+Tab), resize sync, search,
  copy/paste, clickable links (Ctrl+click), disconnect detection and reconnect that keeps scrollback.
- **SSH file download** — open a separate SFTP window from the active terminal, browse remote files and local
  folders, then drag a remote file to a local folder; replacing an existing file requires confirmation.
- **Info panel** (toggle) — host, port, user, auth, status, host-key fingerprint.
- Dark / light / system theme, loading / empty / error states.

## Data & security

All data is **local** (no server):

| OS | Location |
|---|---|
| Windows | `%APPDATA%\com.devnest.app\` |
| macOS | `~/Library/Application Support/com.devnest.app/` |

- `devnest.db` (SQLite, WAL) — connections, groups, known hosts, encrypted secrets.
- `backups/` — automatic DB copy before each schema migration (newest 5 kept).
- Logs: OS app log dir (`…/com.devnest.app/logs`), rotated daily, no secrets.

Secrets (passwords, passphrases, stored private keys) are encrypted with **XChaCha20-Poly1305**.
The 256-bit master key lives in the **OS keychain** (Windows Credential Manager / macOS Keychain).
Secrets are decrypted only in the Rust backend and are never sent to the UI. IDs are UUID v7 to
prepare for Export/Import (Phase 3.5).

## Development

Prerequisites: Node 24+, Rust stable (MSVC toolchain on Windows), WebView2 (Windows 11 has it).

```bash
npm install
npm run tauri dev        # run the app
npm run check            # typecheck + lint + frontend unit tests
npm run tauri build      # installers (MSI/NSIS on Windows, DMG on macOS)
```

Rust tests (unit + SSH integration against a throw-away OpenSSH container, requires Docker):

```bash
npm run test:rust        # = bash scripts/test-ssh.sh
```

> **Windows — Smart App Control:** if *Smart App Control* is **On**, Windows blocks every
> unsigned executable produced locally (Cargo build scripts, the Tailwind native addon, the app
> itself): `An Application Control policy has blocked this file (os error 4551)`.
> Turn it off in *Windows Security → App & browser control → Smart App Control* to develop locally,
> or build in CI / Docker.

## Releases & auto-update

Installed copies check GitHub Releases (`latest.json`) 5 s after start and every 6 h, and via the
↻ button in the status bar. Updates are downloaded, **verified with the update signing key** and
installed; user data (`%APPDATA%\com.devnest.app`, keychain) is kept.

Publish a new version (working tree must be clean):

```bash
npm run release:patch     # or release:minor / release:major
```

This bumps `package.json` (the single source of the app version), commits, tags `vX.Y.Z` and
pushes. [release.yml](.github/workflows/release.yml) builds Windows + macOS, signs the update
bundles and creates a **draft** GitHub Release. Review it and click **Publish release**; only then
do installed apps see the update.

One-time setup — GitHub → *Settings → Secrets and variables → Actions*:

| Secret | Value |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | content of `~/.tauri/devnest-updater.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | content of `~/.tauri/devnest-updater.password.txt` |

The public key is in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`). **Back up the private
key and password**: if they are lost, already-installed apps can never be updated again. The update
key is not a code-signing certificate (SmartScreen still warns on first manual install).

## Project layout

```text
src/                      Vue frontend
  components/             sidebar, connections, terminal, tabs, info, dialogs, common
  modules/                connections & ssh (typed API wrappers, validation, prompt flow)
  stores/                 Pinia: connections, sessions, tabs, ui, dialogs, toasts, app
  layouts/AppLayout.vue
src-tauri/src/            Rust backend
  commands/               Tauri commands (the only API surface of the UI)
  database/               SQLite, migrations, repositories
  services/               connection use-cases (secrets + credentials)
  ssh/                    client (connect/auth/host key), terminal (PTY), manager (lifecycle)
  vault.rs                envelope encryption, OS keychain
  secret.rs               redacting, zeroizing secret type
scripts/test-ssh.sh       dockerized Rust test-suite incl. SSH integration tests
```
