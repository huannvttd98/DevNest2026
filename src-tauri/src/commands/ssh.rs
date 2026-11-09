use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::ipc::{Channel, InvokeResponseBody};

use crate::database::connections as repo;
use crate::database::known_hosts::{self, KnownHost};
use crate::error::{AppError, AppResult};
use crate::services::connections::{self as service, CredentialOverrides, SaveConnectionInput};
use crate::ssh::client::{self, HostKeyInfo};
use crate::ssh::manager::SessionInfo;
use crate::ssh::terminal::{TerminalEvent, TerminalSink};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct TestResult {
    pub elapsed_ms: u128,
    pub host_key: Option<HostKeyInfo>,
}

/// Connect + authenticate with the form values, then disconnect immediately.
#[tauri::command]
pub async fn ssh_test(
    state: State<'_, AppState>,
    connection_id: Option<String>,
    input: SaveConnectionInput,
    overrides: Option<CredentialOverrides>,
) -> AppResult<TestResult> {
    let overrides = overrides.unwrap_or_default();
    let (target, creds) = service::test_params(
        &state.db,
        state.vault()?,
        connection_id.as_deref(),
        &input,
        &overrides,
    )?;
    let known = known_hosts::for_host(&*state.db.conn()?, &target.host, target.port)?;
    let started = Instant::now();
    let connected = client::connect(&target, &creds, known, None).await?;
    let _ = connected
        .handle
        .disconnect(russh::Disconnect::ByApplication, "", "en")
        .await;
    tracing::info!(host = %target.host, "connection test succeeded");
    Ok(TestResult {
        elapsed_ms: started.elapsed().as_millis(),
        host_key: connected.host_key,
    })
}

#[derive(Debug, Deserialize)]
pub struct TrustHostKey {
    pub host: String,
    pub port: u16,
    pub key_type: String,
    pub fingerprint: String,
}

/// Trust a server host key (after the user confirmed the fingerprint).
#[tauri::command]
pub async fn known_host_trust(
    state: State<'_, AppState>,
    key: TrustHostKey,
    replace: bool,
) -> AppResult<()> {
    if key.fingerprint.trim().is_empty() || key.host.trim().is_empty() {
        return Err(AppError::validation("Invalid host key"));
    }
    let mut conn = state.db.conn()?;
    known_hosts::trust(
        &mut conn,
        &KnownHost {
            host: key.host,
            port: key.port,
            key_type: key.key_type,
            fingerprint: key.fingerprint,
        },
        replace,
    )?;
    tracing::info!(replace, "host key trusted");
    Ok(())
}

#[tauri::command]
pub async fn ssh_sessions(state: State<'_, AppState>) -> AppResult<Vec<SessionInfo>> {
    state.ssh.sessions()
}

#[tauri::command]
pub async fn ssh_disconnect(state: State<'_, AppState>, connection_id: String) -> AppResult<()> {
    state.ssh.disconnect(&connection_id).await
}

// ---- terminal -------------------------------------------------------------

struct ChannelSink(Channel<InvokeResponseBody>);

impl TerminalSink for ChannelSink {
    fn data(&self, bytes: Vec<u8>) -> bool {
        self.0.send(InvokeResponseBody::Raw(bytes)).is_ok()
    }

    fn event(&self, event: TerminalEvent) -> bool {
        match serde_json::to_string(&event) {
            Ok(json) => self.0.send(InvokeResponseBody::Json(json)).is_ok(),
            Err(_) => false,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OpenTerminalResult {
    pub terminal_id: String,
    pub session: Option<SessionInfo>,
}

/// Open an interactive shell. Connects first when the connection is not live.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn terminal_open(
    state: State<'_, AppState>,
    connection_id: String,
    cols: u32,
    rows: u32,
    overrides: Option<CredentialOverrides>,
    save_overrides: Option<bool>,
    on_event: Channel<InvokeResponseBody>,
) -> AppResult<OpenTerminalResult> {
    let overrides = overrides.unwrap_or_default();
    let rec = repo::get(&*state.db.conn()?, &connection_id)?;
    let target = service::target_of(&rec);
    let known = service::known_hosts_for(&state.db, &rec)?;
    let vault = state.vault()?;

    let (live, fresh) = state
        .ssh
        .ensure_connected(&connection_id, &target, known, || {
            service::credentials_for(&state.db, vault, &rec, &overrides)
        })
        .await?;

    if fresh {
        let _ = repo::touch_connected(&*state.db.conn()?, &connection_id);
        if save_overrides.unwrap_or(false) {
            service::store_prompted(&state.db, vault, &connection_id, &overrides)?;
        }
    }

    let terminal_id = crate::database::new_id();
    state
        .ssh
        .open_terminal(
            terminal_id.clone(),
            &live,
            &connection_id,
            cols,
            rows,
            Arc::new(ChannelSink(on_event)),
        )
        .await?;

    let session = state
        .ssh
        .sessions()?
        .into_iter()
        .find(|s| s.connection_id == connection_id);
    Ok(OpenTerminalResult {
        terminal_id,
        session,
    })
}

/// Send keyboard input. `binary` input (xterm `onBinary`) is a latin-1 string where
/// every char is one byte.
#[tauri::command]
pub async fn terminal_write(
    state: State<'_, AppState>,
    terminal_id: String,
    data: String,
    binary: Option<bool>,
) -> AppResult<()> {
    if binary.unwrap_or(false) {
        let bytes: Vec<u8> = data.chars().map(|c| (c as u32 & 0xff) as u8).collect();
        state.ssh.write(&terminal_id, &bytes).await
    } else {
        state.ssh.write(&terminal_id, data.as_bytes()).await
    }
}

#[tauri::command]
pub async fn terminal_resize(
    state: State<'_, AppState>,
    terminal_id: String,
    cols: u32,
    rows: u32,
) -> AppResult<()> {
    state.ssh.resize(&terminal_id, cols, rows).await
}

#[tauri::command]
pub async fn terminal_close(state: State<'_, AppState>, terminal_id: String) -> AppResult<()> {
    state.ssh.close_terminal(&terminal_id).await
}
