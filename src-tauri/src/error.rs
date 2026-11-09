//! Unified application error.
//!
//! Every Tauri command returns `Result<T, AppError>`. The error is serialized to the
//! frontend as `{ code, message, details? }` so the UI can react to specific codes
//! (e.g. prompt for a password, show the host-key dialog).
//!
//! IMPORTANT: messages and details must never contain secrets.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    Internal,
    Database,
    Vault,
    NotFound,
    Validation,
    /// Host unreachable / DNS / TCP failure.
    SshConnect,
    SshTimeout,
    SshAuthFailed,
    /// Password auth selected but no password is stored and none was supplied.
    AuthPasswordRequired,
    /// The private key is encrypted and no passphrase is stored / supplied.
    KeyPassphraseRequired,
    KeyInvalid,
    KeyFileNotFound,
    /// Server host key is not in known_hosts yet (TOFU prompt).
    HostKeyUnknown,
    /// Server host key differs from the stored fingerprint (possible MITM).
    HostKeyChanged,
    SshDisconnected,
    SshChannel,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn not_found(what: &str) -> Self {
        Self::new(ErrorCode::NotFound, format!("{what} not found"))
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Validation, message)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        match e {
            rusqlite::Error::QueryReturnedNoRows => {
                Self::new(ErrorCode::NotFound, "Record not found")
            }
            other => Self::new(ErrorCode::Database, format!("Database error: {other}")),
        }
    }
}

impl From<rusqlite_migration::Error> for AppError {
    fn from(e: rusqlite_migration::Error) -> Self {
        Self::new(ErrorCode::Database, format!("Migration error: {e}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::internal(format!("I/O error: {e}"))
    }
}

pub type AppResult<T> = Result<T, AppError>;
