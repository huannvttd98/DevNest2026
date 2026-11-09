//! Interactive shell (PTY) over an SSH channel.

use std::sync::Arc;
use std::time::Duration;

use russh::ChannelMsg;
use russh::client::{Handle, Msg};
use russh::{ChannelReadHalf, ChannelWriteHalf, Pty};
use serde::Serialize;
use tokio::task::JoinHandle;

use super::client::ClientHandler;
use crate::error::{AppError, AppResult, ErrorCode};

pub const TERM: &str = "xterm-256color";
/// Flush buffered output at least this often while data keeps arriving.
const FLUSH_INTERVAL: Duration = Duration::from_millis(8);
/// ...or as soon as the buffer reaches this size.
const FLUSH_BYTES: usize = 64 * 1024;

/// Control events sent to the frontend (output bytes are sent separately, raw).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerminalEvent {
    /// The remote shell exited.
    Exit {
        code: Option<u32>,
        signal: Option<String>,
    },
    /// The channel is closed (shell exit, connection lost or closed by user).
    Closed { reason: Option<String> },
}

/// Destination of terminal output. Implemented over a Tauri channel in production
/// and over an mpsc in tests. Returns `false` once the receiver is gone.
pub trait TerminalSink: Send + Sync + 'static {
    fn data(&self, bytes: Vec<u8>) -> bool;
    fn event(&self, event: TerminalEvent) -> bool;
}

pub struct Terminal {
    pub connection_id: String,
    writer: ChannelWriteHalf<Msg>,
    reader: JoinHandle<()>,
}

impl Terminal {
    /// Open a session channel, request a PTY + shell and start pumping output to `sink`.
    /// `on_closed` is invoked (once) when the reader loop ends.
    pub async fn open(
        connection_id: String,
        handle: &Handle<ClientHandler>,
        cols: u32,
        rows: u32,
        sink: Arc<dyn TerminalSink>,
        on_closed: impl FnOnce() + Send + 'static,
    ) -> AppResult<Self> {
        let channel = handle.channel_open_session().await.map_err(|e| {
            AppError::new(ErrorCode::SshChannel, format!("Cannot open session: {e}"))
        })?;
        let modes = [
            (Pty::ECHO, 1),
            (Pty::TTY_OP_ISPEED, 14400),
            (Pty::TTY_OP_OSPEED, 14400),
        ];
        channel
            .request_pty(true, TERM, cols.max(1), rows.max(1), 0, 0, &modes)
            .await
            .map_err(|e| {
                AppError::new(ErrorCode::SshChannel, format!("PTY request failed: {e}"))
            })?;
        channel.request_shell(true).await.map_err(|e| {
            AppError::new(ErrorCode::SshChannel, format!("Shell request failed: {e}"))
        })?;

        let (read, writer) = channel.split();
        let reader = tokio::spawn(pump(read, sink, on_closed));
        Ok(Self {
            connection_id,
            writer,
            reader,
        })
    }

    pub async fn write(&self, data: &[u8]) -> AppResult<()> {
        self.writer
            .data_bytes(data.to_vec())
            .await
            .map_err(|e| AppError::new(ErrorCode::SshDisconnected, format!("Write failed: {e}")))
    }

    pub async fn resize(&self, cols: u32, rows: u32) -> AppResult<()> {
        self.writer
            .window_change(cols.max(1), rows.max(1), 0, 0)
            .await
            .map_err(|e| AppError::new(ErrorCode::SshDisconnected, format!("Resize failed: {e}")))
    }

    /// Close the channel. Best effort: the connection may already be gone.
    pub async fn close(&self) {
        let _ = self.writer.eof().await;
        let _ = self.writer.close().await;
        // Give the reader a moment to observe the close, then stop it.
        let _ = tokio::time::timeout(Duration::from_millis(300), async {
            while !self.reader.is_finished() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        self.reader.abort();
    }
}

async fn pump(mut read: ChannelReadHalf, sink: Arc<dyn TerminalSink>, on_closed: impl FnOnce()) {
    let mut buf: Vec<u8> = Vec::with_capacity(16 * 1024);
    let mut reason: Option<String> = None;

    loop {
        let msg = if buf.is_empty() {
            read.wait().await
        } else {
            match tokio::time::timeout(FLUSH_INTERVAL, read.wait()).await {
                Ok(m) => m,
                Err(_) => {
                    if !sink.data(std::mem::take(&mut buf)) {
                        break;
                    }
                    continue;
                }
            }
        };

        match msg {
            Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                buf.extend_from_slice(&data);
                if buf.len() >= FLUSH_BYTES && !sink.data(std::mem::take(&mut buf)) {
                    break;
                }
            }
            Some(ChannelMsg::ExitStatus { exit_status }) => {
                flush(&mut buf, sink.as_ref());
                sink.event(TerminalEvent::Exit {
                    code: Some(exit_status),
                    signal: None,
                });
                reason = Some(format!("Shell exited with code {exit_status}"));
            }
            Some(ChannelMsg::ExitSignal { signal_name, .. }) => {
                flush(&mut buf, sink.as_ref());
                let sig = format!("{signal_name:?}");
                sink.event(TerminalEvent::Exit {
                    code: None,
                    signal: Some(sig.clone()),
                });
                reason = Some(format!("Shell terminated by signal {sig}"));
            }
            Some(ChannelMsg::Close) => {
                reason.get_or_insert_with(|| "Session closed by server".into());
                break;
            }
            None => {
                reason.get_or_insert_with(|| "Connection lost".into());
                break;
            }
            Some(_) => {}
        }
    }

    flush(&mut buf, sink.as_ref());
    sink.event(TerminalEvent::Closed { reason });
    on_closed();
}

fn flush(buf: &mut Vec<u8>, sink: &dyn TerminalSink) {
    if !buf.is_empty() {
        sink.data(std::mem::take(buf));
    }
}
