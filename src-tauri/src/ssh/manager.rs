//! Lifecycle of live SSH connections and their terminal channels.
//!
//! * One TCP/SSH connection per saved connection, shared by all its terminal tabs
//!   (and, in Phase 3, its SFTP session).
//! * A connection is released automatically when its last terminal closes.
//! * Connection loss is reported through the `StatusListener`; every terminal of that
//!   connection receives a `Closed` event from its reader loop.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use russh::client::Handle;
use russh_sftp::client::SftpSession;
use serde::Serialize;
use tokio::io::{AsyncWriteExt, copy};

use super::client::{self, ClientHandler, Credentials, HostKeyInfo, Target};
use super::terminal::{Terminal, TerminalSink};
use crate::database::known_hosts::KnownHost;
use crate::error::{AppError, AppResult, ErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Connecting,
    Connected,
    Disconnected,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusEvent {
    pub connection_id: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub type StatusListener = Arc<dyn Fn(StatusEvent) + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub connection_id: String,
    pub connected_at: String,
    pub host_key: Option<HostKeyInfo>,
    pub terminals: usize,
}

#[derive(Debug, Serialize)]
pub struct RemoteDirectory {
    pub path: String,
    pub entries: Vec<RemoteFileEntry>,
}

#[derive(Debug, Serialize)]
pub struct RemoteFileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

struct LiveConnection {
    generation: u64,
    handle: Handle<ClientHandler>,
    connected_at: String,
    host_key: Option<HostKeyInfo>,
}

#[derive(Default)]
struct State {
    connections: HashMap<String, Arc<LiveConnection>>,
    terminals: HashMap<String, Arc<Terminal>>,
    connect_locks: HashMap<String, Arc<tokio::sync::Mutex<()>>>,
}

struct Shared {
    state: Mutex<State>,
    generation: AtomicU64,
    listener: StatusListener,
}

#[derive(Clone)]
pub struct SshManager {
    shared: Arc<Shared>,
}

impl SshManager {
    pub fn new(listener: StatusListener) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State::default()),
                generation: AtomicU64::new(1),
                listener,
            }),
        }
    }

    fn state(&self) -> AppResult<std::sync::MutexGuard<'_, State>> {
        self.shared
            .state
            .lock()
            .map_err(|_| AppError::internal("session state lock poisoned"))
    }

    fn emit(&self, connection_id: &str, status: Status, message: Option<String>) {
        (self.shared.listener)(StatusEvent {
            connection_id: connection_id.to_string(),
            status,
            message,
        });
    }

    pub fn sessions(&self) -> AppResult<Vec<SessionInfo>> {
        let s = self.state()?;
        Ok(s.connections
            .iter()
            .filter(|(_, c)| !c.handle.is_closed())
            .map(|(id, c)| SessionInfo {
                connection_id: id.clone(),
                connected_at: c.connected_at.clone(),
                host_key: c.host_key.clone(),
                terminals: s
                    .terminals
                    .values()
                    .filter(|t| &t.connection_id == id)
                    .count(),
            })
            .collect())
    }

    /// Return the live connection, connecting first if needed.
    /// `creds` is only evaluated when a new connection must be established.
    pub async fn ensure_connected<F>(
        &self,
        connection_id: &str,
        target: &Target,
        known: Vec<KnownHost>,
        creds: F,
    ) -> AppResult<(Arc<LiveConnectionHandle>, bool)>
    where
        F: FnOnce() -> AppResult<Credentials>,
    {
        let lock = {
            let mut s = self.state()?;
            s.connect_locks
                .entry(connection_id.to_string())
                .or_default()
                .clone()
        };
        let _guard = lock.lock().await;

        if let Some(live) = self.live(connection_id)? {
            return Ok((Arc::new(LiveConnectionHandle(live)), false));
        }

        let creds = creds()?;
        self.emit(connection_id, Status::Connecting, None);
        let generation = self.shared.generation.fetch_add(1, Ordering::Relaxed);
        let on_disconnect = self.disconnect_callback(connection_id.to_string(), generation);

        match client::connect(target, &creds, known, Some(on_disconnect)).await {
            Ok(connected) => {
                let live = Arc::new(LiveConnection {
                    generation,
                    handle: connected.handle,
                    connected_at: crate::database::now(),
                    host_key: connected.host_key,
                });
                self.state()?
                    .connections
                    .insert(connection_id.to_string(), live.clone());
                tracing::info!(connection_id, "ssh connected");
                self.emit(connection_id, Status::Connected, None);
                Ok((Arc::new(LiveConnectionHandle(live)), true))
            }
            Err(e) => {
                // Prompts (password / passphrase / host key) are not "errors" for the status dot.
                let status = match e.code {
                    ErrorCode::AuthPasswordRequired
                    | ErrorCode::KeyPassphraseRequired
                    | ErrorCode::HostKeyUnknown
                    | ErrorCode::HostKeyChanged => Status::Disconnected,
                    _ => Status::Error,
                };
                self.emit(connection_id, status, Some(e.message.clone()));
                Err(e)
            }
        }
    }

    fn live(&self, connection_id: &str) -> AppResult<Option<Arc<LiveConnection>>> {
        let mut s = self.state()?;
        match s.connections.get(connection_id) {
            Some(c) if !c.handle.is_closed() => Ok(Some(c.clone())),
            Some(_) => {
                s.connections.remove(connection_id);
                Ok(None)
            }
            None => Ok(None),
        }
    }

    fn disconnect_callback(
        &self,
        connection_id: String,
        generation: u64,
    ) -> client::DisconnectCallback {
        let weak: Weak<Shared> = Arc::downgrade(&self.shared);
        Arc::new(move |message: Option<String>| {
            let Some(shared) = weak.upgrade() else { return };
            let removed = {
                let Ok(mut s) = shared.state.lock() else {
                    return;
                };
                match s.connections.get(&connection_id) {
                    Some(c) if c.generation == generation => {
                        s.connections.remove(&connection_id);
                        true
                    }
                    _ => false,
                }
            };
            if removed {
                (shared.listener)(StatusEvent {
                    connection_id: connection_id.clone(),
                    status: Status::Disconnected,
                    message,
                });
            }
        })
    }

    /// Open a new terminal on a live connection.
    pub async fn open_terminal(
        &self,
        terminal_id: String,
        connection: &LiveConnectionHandle,
        connection_id: &str,
        cols: u32,
        rows: u32,
        sink: Arc<dyn TerminalSink>,
    ) -> AppResult<()> {
        let me = self.clone();
        let tid = terminal_id.clone();
        let terminal = Terminal::open(
            connection_id.to_string(),
            &connection.0.handle,
            cols,
            rows,
            sink,
            move || {
                // Reader ended: forget the terminal and release the connection if unused.
                let me2 = me.clone();
                tokio::spawn(async move {
                    me2.forget_terminal(&tid).await;
                });
            },
        )
        .await?;
        self.state()?
            .terminals
            .insert(terminal_id, Arc::new(terminal));
        Ok(())
    }

    fn terminal(&self, id: &str) -> AppResult<Arc<Terminal>> {
        self.state()?
            .terminals
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::new(ErrorCode::SshDisconnected, "Terminal session is closed"))
    }

    pub async fn write(&self, terminal_id: &str, data: &[u8]) -> AppResult<()> {
        self.terminal(terminal_id)?.write(data).await
    }

    pub async fn resize(&self, terminal_id: &str, cols: u32, rows: u32) -> AppResult<()> {
        self.terminal(terminal_id)?.resize(cols, rows).await
    }

    pub async fn list_remote_dir(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> AppResult<RemoteDirectory> {
        let live = self.live(connection_id)?.ok_or_else(|| {
            AppError::new(ErrorCode::SshDisconnected, "SSH session is not connected")
        })?;
        let channel = live.handle.channel_open_session().await.map_err(|e| {
            AppError::new(
                ErrorCode::SshChannel,
                format!("Cannot open SFTP channel: {e}"),
            )
        })?;
        channel.request_subsystem(true, "sftp").await.map_err(|e| {
            AppError::new(
                ErrorCode::SshChannel,
                format!("Cannot start SFTP subsystem: {e}"),
            )
        })?;
        let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| {
            AppError::new(
                ErrorCode::SshChannel,
                format!("Cannot initialize SFTP: {e}"),
            )
        })?;
        let path = if remote_path.trim().is_empty() {
            "."
        } else {
            remote_path
        };
        let path = sftp.canonicalize(path).await.map_err(|e| {
            AppError::new(
                ErrorCode::SshChannel,
                format!("Cannot open remote folder: {e}"),
            )
        })?;
        let mut entries = sftp
            .read_dir(&path)
            .await
            .map_err(|e| {
                AppError::new(
                    ErrorCode::SshChannel,
                    format!("Cannot list remote folder: {e}"),
                )
            })?
            .map(|entry| {
                let metadata = entry.metadata();
                RemoteFileEntry {
                    name: entry.file_name(),
                    path: entry.path(),
                    is_dir: metadata.is_dir(),
                    size: metadata.size,
                }
            })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        sftp.close().await.map_err(|e| {
            AppError::new(
                ErrorCode::SshChannel,
                format!("Cannot close SFTP session: {e}"),
            )
        })?;
        Ok(RemoteDirectory { path, entries })
    }

    /// Download a remote file over SFTP using an already-live SSH connection.
    pub async fn download_file(
        &self,
        connection_id: &str,
        remote_path: &str,
        local_directory: &std::path::Path,
        file_name: &str,
        overwrite: bool,
    ) -> AppResult<String> {
        if remote_path.trim().is_empty() {
            return Err(AppError::validation("Remote file path is required"));
        }
        if file_name.is_empty()
            || file_name == "."
            || file_name == ".."
            || file_name.chars().any(|c| matches!(c, '/' | '\\' | '\0'))
        {
            return Err(AppError::validation("Invalid local file name"));
        }
        let local_directory = tokio::fs::canonicalize(local_directory).await?;
        if !tokio::fs::metadata(&local_directory).await?.is_dir() {
            return Err(AppError::validation(
                "The selected local destination is not a folder",
            ));
        }
        let local_path = local_directory.join(file_name);
        if !overwrite && tokio::fs::try_exists(&local_path).await? {
            return Err(AppError::new(
                ErrorCode::FileExists,
                "The selected local file already exists",
            ));
        }

        let temporary = tempfile::NamedTempFile::new_in(&local_directory).map_err(|e| {
            AppError::internal(format!("Cannot create a temporary download file: {e}"))
        })?;

        let live = self.live(connection_id)?.ok_or_else(|| {
            AppError::new(ErrorCode::SshDisconnected, "SSH session is not connected")
        })?;
        let channel = live.handle.channel_open_session().await.map_err(|e| {
            AppError::new(
                ErrorCode::SshChannel,
                format!("Cannot open SFTP channel: {e}"),
            )
        })?;
        channel.request_subsystem(true, "sftp").await.map_err(|e| {
            AppError::new(
                ErrorCode::SshChannel,
                format!("Cannot start SFTP subsystem: {e}"),
            )
        })?;
        let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| {
            AppError::new(
                ErrorCode::SshChannel,
                format!("Cannot initialize SFTP: {e}"),
            )
        })?;
        let mut remote = sftp.open(remote_path).await.map_err(|e| {
            AppError::new(
                ErrorCode::SshChannel,
                format!("Cannot open remote file: {e}"),
            )
        })?;

        let std_file = temporary
            .reopen()
            .map_err(|e| AppError::internal(format!("Cannot write the downloaded file: {e}")))?;
        let mut local = tokio::fs::File::from_std(std_file);
        copy(&mut remote, &mut local).await.map_err(|e| {
            AppError::new(ErrorCode::SshChannel, format!("File download failed: {e}"))
        })?;
        local
            .flush()
            .await
            .map_err(|e| AppError::internal(format!("Cannot flush the downloaded file: {e}")))?;
        local
            .sync_all()
            .await
            .map_err(|e| AppError::internal(format!("Cannot save the downloaded file: {e}")))?;
        drop(local);

        remote.close().await.map_err(|e| {
            AppError::new(
                ErrorCode::SshChannel,
                format!("Cannot close remote file: {e}"),
            )
        })?;
        sftp.close().await.map_err(|e| {
            AppError::new(
                ErrorCode::SshChannel,
                format!("Cannot close SFTP session: {e}"),
            )
        })?;

        let persisted = if overwrite {
            temporary.persist(&local_path)
        } else {
            temporary.persist_noclobber(&local_path)
        };
        persisted.map_err(|e| {
            if e.error.kind() == std::io::ErrorKind::AlreadyExists {
                AppError::new(
                    ErrorCode::FileExists,
                    "The selected local file already exists",
                )
            } else {
                AppError::internal(format!("Cannot save the downloaded file: {}", e.error))
            }
        })?;
        tracing::info!(connection_id, "SFTP file downloaded");
        Ok(local_path.display().to_string())
    }

    pub async fn close_terminal(&self, terminal_id: &str) -> AppResult<()> {
        let term = self.state()?.terminals.remove(terminal_id);
        if let Some(t) = term {
            t.close().await;
            self.release_if_unused(&t.connection_id).await;
        }
        Ok(())
    }

    async fn forget_terminal(&self, terminal_id: &str) {
        let removed = self
            .state()
            .ok()
            .and_then(|mut s| s.terminals.remove(terminal_id));
        if let Some(t) = removed {
            self.release_if_unused(&t.connection_id).await;
        }
    }

    async fn release_if_unused(&self, connection_id: &str) {
        let unused = self
            .state()
            .map(|s| {
                !s.terminals
                    .values()
                    .any(|t| t.connection_id == connection_id)
            })
            .unwrap_or(false);
        if unused {
            let _ = self.disconnect(connection_id).await;
        }
    }

    /// Close all terminals of a connection and disconnect it.
    pub async fn disconnect(&self, connection_id: &str) -> AppResult<()> {
        let (live, terminals) = {
            let mut s = self.state()?;
            let live = s.connections.remove(connection_id);
            let ids: Vec<String> = s
                .terminals
                .iter()
                .filter(|(_, t)| t.connection_id == connection_id)
                .map(|(id, _)| id.clone())
                .collect();
            let terms: Vec<_> = ids.iter().filter_map(|id| s.terminals.remove(id)).collect();
            (live, terms)
        };
        for t in terminals {
            t.close().await;
        }
        if let Some(live) = live {
            let _ = live
                .handle
                .disconnect(russh::Disconnect::ByApplication, "", "en")
                .await;
            tracing::info!(connection_id, "ssh disconnected");
            self.emit(connection_id, Status::Disconnected, None);
        }
        Ok(())
    }

    pub async fn disconnect_all(&self) {
        let ids: Vec<String> = self
            .state()
            .map(|s| s.connections.keys().cloned().collect())
            .unwrap_or_default();
        for id in ids {
            let _ = self.disconnect(&id).await;
        }
    }
}

/// Opaque handle to a live connection, passed between manager calls.
pub struct LiveConnectionHandle(Arc<LiveConnection>);
