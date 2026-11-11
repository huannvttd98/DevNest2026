//! Low-level SSH connect + authenticate on top of `russh`.

use std::sync::Arc;
use std::time::Duration;

use russh::client::{self, AuthResult, DisconnectReason, Handle};
use russh::keys::{
    HashAlg, PrivateKey, PrivateKeyWithHashAlg, PublicKeyOrCertificate, decode_secret_key,
};
use serde::Serialize;
use serde_json::json;
use zeroize::Zeroizing;

use crate::database::known_hosts::KnownHost;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::secret::Secret;
use crate::ssh::options::SshOptions;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);
const KEEPALIVE_MAX: usize = 3;
/// Private key files larger than this are rejected (real keys are a few KB).
const MAX_KEY_FILE_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone)]
pub struct Target {
    pub host: String,
    pub port: u16,
    pub username: String,
    /// Algorithm overrides (`HostKeyAlgorithms=+ssh-rsa`, ...).
    pub options: SshOptions,
}

#[derive(Debug, Clone)]
pub enum KeyMaterial {
    Path(String),
    Content(Secret),
}

#[derive(Debug, Clone)]
pub enum Credentials {
    Password(Secret),
    Key {
        key: KeyMaterial,
        passphrase: Option<Secret>,
    },
}

/// Information about the server host key presented to the UI.
#[derive(Debug, Clone, Serialize)]
pub struct HostKeyInfo {
    pub host: String,
    pub port: u16,
    pub key_type: String,
    pub fingerprint: String,
    /// Previously trusted fingerprint (only for a changed key).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_fingerprint: Option<String>,
}

#[derive(Debug)]
pub enum HandlerError {
    Russh(russh::Error),
    HostKeyUnknown(HostKeyInfo),
    HostKeyChanged(HostKeyInfo),
}

impl From<russh::Error> for HandlerError {
    fn from(e: russh::Error) -> Self {
        HandlerError::Russh(e)
    }
}

pub type DisconnectCallback = Arc<dyn Fn(Option<String>) + Send + Sync>;

pub struct ClientHandler {
    host: String,
    port: u16,
    known: Vec<KnownHost>,
    /// Host key the server presented and we accepted (shared with the caller).
    accepted: Arc<std::sync::Mutex<Option<HostKeyInfo>>>,
    on_disconnect: Option<DisconnectCallback>,
}

impl client::Handler for ClientHandler {
    type Error = HandlerError;

    async fn check_server_key(
        &mut self,
        server_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let pk = server_key.public_key();
        let key_type = pk.algorithm().as_str().to_string();
        let fingerprint = pk.fingerprint(HashAlg::Sha256).to_string();
        let info = HostKeyInfo {
            host: self.host.clone(),
            port: self.port,
            key_type: key_type.clone(),
            fingerprint: fingerprint.clone(),
            previous_fingerprint: None,
        };
        if self
            .known
            .iter()
            .any(|k| k.key_type == key_type && k.fingerprint == fingerprint)
        {
            if let Ok(mut slot) = self.accepted.lock() {
                *slot = Some(info);
            }
            return Ok(true);
        }
        if let Some(prev) = self.known.iter().find(|k| k.key_type == key_type) {
            // Same key type, different fingerprint – possible MITM.
            return Err(HandlerError::HostKeyChanged(HostKeyInfo {
                previous_fingerprint: Some(format!("{} ({})", prev.fingerprint, prev.key_type)),
                ..info
            }));
        }
        // No key of this type is known yet (first connection, or the algorithm options now
        // select another host key type) – ask the user like OpenSSH does.
        Err(HandlerError::HostKeyUnknown(info))
    }

    async fn disconnected(
        &mut self,
        reason: DisconnectReason<Self::Error>,
    ) -> Result<(), Self::Error> {
        let message = match &reason {
            DisconnectReason::ReceivedDisconnect(info) => {
                let m = format!("{info:?}");
                tracing::info!(host = %self.host, "server closed the connection");
                Some(m)
            }
            DisconnectReason::Error(e) => {
                tracing::warn!(host = %self.host, error = ?e, "connection lost");
                Some(describe_handler_error(e))
            }
        };
        if let Some(cb) = &self.on_disconnect {
            cb(message);
        }
        match reason {
            DisconnectReason::ReceivedDisconnect(_) => Ok(()),
            DisconnectReason::Error(e) => Err(e),
        }
    }
}

fn describe_handler_error(e: &HandlerError) -> String {
    match e {
        HandlerError::Russh(r) => r.to_string(),
        HandlerError::HostKeyUnknown(_) => "Unknown host key".into(),
        HandlerError::HostKeyChanged(_) => "Host key changed".into(),
    }
}

pub struct Connected {
    pub handle: Handle<ClientHandler>,
    pub host_key: Option<HostKeyInfo>,
}

/// Decode the private key, distinguishing "passphrase required" / "wrong passphrase".
fn load_key(material: &KeyMaterial, passphrase: Option<&Secret>) -> AppResult<PrivateKey> {
    let content: Zeroizing<String> = match material {
        KeyMaterial::Content(s) => Zeroizing::new(s.expose().to_string()),
        KeyMaterial::Path(p) => read_key_file(p)?,
    };
    match decode_secret_key(&content, None) {
        Ok(k) => Ok(k),
        Err(russh::keys::Error::KeyIsEncrypted) => match passphrase.filter(|p| !p.is_empty()) {
            None => Err(AppError::new(
                ErrorCode::KeyPassphraseRequired,
                "The private key is protected by a passphrase",
            )),
            Some(p) => decode_secret_key(&content, Some(p.expose())).map_err(|_| {
                AppError::new(
                    ErrorCode::KeyPassphraseRequired,
                    "Incorrect passphrase for the private key",
                )
                .with_details(json!({ "incorrect": true }))
            }),
        },
        // PuTTY / PKCS#8 encrypted keys report other errors without a passphrase.
        Err(first) => match passphrase.filter(|p| !p.is_empty()) {
            Some(p) => decode_secret_key(&content, Some(p.expose())).map_err(|e| {
                AppError::new(
                    ErrorCode::KeyInvalid,
                    format!("Cannot read private key: {e}"),
                )
            }),
            None => Err(AppError::new(
                ErrorCode::KeyInvalid,
                format!("Cannot read private key: {first}"),
            )),
        },
    }
}

/// Expand a leading `~` to the user's home directory.
fn expand_home(path: &str) -> std::path::PathBuf {
    let rest = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\"));
    match (rest, std::env::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => std::path::PathBuf::from(path),
    }
}

pub fn read_key_file(path: &str) -> AppResult<Zeroizing<String>> {
    let expanded = expand_home(path);
    let p = expanded.as_path();
    let meta = std::fs::metadata(p).map_err(|_| {
        AppError::new(
            ErrorCode::KeyFileNotFound,
            format!("Private key file not found: {path}"),
        )
    })?;
    if !meta.is_file() {
        return Err(AppError::new(
            ErrorCode::KeyFileNotFound,
            format!("Not a file: {path}"),
        ));
    }
    if meta.len() > MAX_KEY_FILE_BYTES {
        return Err(AppError::new(
            ErrorCode::KeyInvalid,
            "File is too large to be a private key",
        ));
    }
    let bytes = Zeroizing::new(std::fs::read(p)?);
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| AppError::new(ErrorCode::KeyInvalid, "Private key file is not a text key"))?;
    Ok(Zeroizing::new(text.to_string()))
}

/// Validate that a key (content) can be parsed, without connecting.
pub fn validate_key_content(content: &Secret, passphrase: Option<&Secret>) -> AppResult<()> {
    match load_key(&KeyMaterial::Content(content.clone()), passphrase) {
        Ok(_) => Ok(()),
        // An encrypted key is fine to store; the passphrase can be supplied at connect time.
        Err(e) if e.code == ErrorCode::KeyPassphraseRequired && passphrase.is_none() => Ok(()),
        Err(e) => Err(e),
    }
}

fn map_connect_error(e: HandlerError, target: &Target) -> AppError {
    match e {
        HandlerError::HostKeyUnknown(info) => AppError::new(
            ErrorCode::HostKeyUnknown,
            format!(
                "The authenticity of host '{}' can't be established",
                target.host
            ),
        )
        .with_details(serde_json::to_value(info).unwrap_or_default()),
        HandlerError::HostKeyChanged(info) => AppError::new(
            ErrorCode::HostKeyChanged,
            format!("WARNING: the host key for '{}' has changed!", target.host),
        )
        .with_details(serde_json::to_value(info).unwrap_or_default()),
        HandlerError::Russh(r) => map_russh_error(r, target),
    }
}

pub fn map_russh_error(e: russh::Error, target: &Target) -> AppError {
    use russh::Error as E;
    match e {
        E::IO(io) => AppError::new(
            ErrorCode::SshConnect,
            format!("Cannot connect to {}:{} – {io}", target.host, target.port),
        ),
        E::ConnectionTimeout | E::Elapsed(_) => AppError::new(
            ErrorCode::SshTimeout,
            format!("Connection to {}:{} timed out", target.host, target.port),
        ),
        E::KeepaliveTimeout | E::InactivityTimeout | E::HUP | E::Disconnect => {
            AppError::new(ErrorCode::SshDisconnected, format!("Connection lost: {e}"))
        }
        E::NoAuthMethod | E::NotAuthenticated => AppError::new(
            ErrorCode::SshAuthFailed,
            "No supported authentication method accepted",
        ),
        E::NoCommonAlgo { kind, theirs, .. } => {
            let (option, hint) = match kind {
                russh::AlgorithmKind::Key => ("HostKeyAlgorithms", theirs.first().cloned()),
                russh::AlgorithmKind::Kex => ("KexAlgorithms", theirs.first().cloned()),
                russh::AlgorithmKind::Cipher => ("Ciphers", theirs.first().cloned()),
                russh::AlgorithmKind::Mac => ("MACs", theirs.first().cloned()),
                russh::AlgorithmKind::Compression => ("Compression", None),
            };
            let suggestion = hint
                .map(|h| format!(" Try adding the SSH option `{option}=+{h}` (Edit → Advanced)."))
                .unwrap_or_default();
            AppError::new(
                ErrorCode::SshConnect,
                format!(
                    "The server only offers legacy {kind:?} algorithms: {}.{suggestion}",
                    theirs.join(", ")
                ),
            )
            .with_details(json!({ "option": option, "server_offers": theirs }))
        }
        other => AppError::new(ErrorCode::SshConnect, format!("SSH error: {other}")),
    }
}

/// Connect and authenticate. `known` are the trusted keys for this host:port.
pub async fn connect(
    target: &Target,
    creds: &Credentials,
    known: Vec<KnownHost>,
    on_disconnect: Option<DisconnectCallback>,
) -> AppResult<Connected> {
    // Decode the key first: fail fast on passphrase problems before touching the network.
    let key = match creds {
        Credentials::Key { key, passphrase } => Some(load_key(key, passphrase.as_ref())?),
        Credentials::Password(p) if p.is_empty() => {
            return Err(AppError::new(
                ErrorCode::AuthPasswordRequired,
                "Password is required",
            ));
        }
        Credentials::Password(_) => None,
    };

    let config = Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(KEEPALIVE_INTERVAL),
        keepalive_max: KEEPALIVE_MAX,
        nodelay: true,
        preferred: target.options.preferred(),
        ..Default::default()
    });
    let accepted = Arc::new(std::sync::Mutex::new(None));
    let handler = ClientHandler {
        host: target.host.clone(),
        port: target.port,
        known,
        accepted: accepted.clone(),
        on_disconnect,
    };
    let host = target
        .host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();
    let addr = (host, target.port);

    let fut = async {
        let mut handle = client::connect(config, addr, handler)
            .await
            .map_err(|e| map_connect_error(e, target))?;

        let result = match (creds, key) {
            (Credentials::Password(p), _) => handle
                .authenticate_password(target.username.clone(), p.expose().to_string())
                .await
                .map_err(|e| map_russh_error(e, target))?,
            (Credentials::Key { .. }, Some(key)) => {
                let hash = handle
                    .best_supported_rsa_hash()
                    .await
                    .map_err(|e| map_russh_error(e, target))?
                    .flatten();
                handle
                    .authenticate_publickey(
                        target.username.clone(),
                        PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                    )
                    .await
                    .map_err(|e| map_russh_error(e, target))?
            }
            (Credentials::Key { .. }, None) => unreachable!("key decoded above"),
        };

        match result {
            AuthResult::Success => Ok(handle),
            AuthResult::Failure { .. } => {
                let _ = handle
                    .disconnect(russh::Disconnect::ByApplication, "", "en")
                    .await;
                let what = match creds {
                    Credentials::Password(_) => "password",
                    Credentials::Key { .. } => "private key",
                };
                Err(AppError::new(
                    ErrorCode::SshAuthFailed,
                    format!(
                        "Authentication failed for user '{}' ({what})",
                        target.username
                    ),
                ))
            }
        }
    };

    let handle = tokio::time::timeout(CONNECT_TIMEOUT, fut)
        .await
        .map_err(|_| {
            AppError::new(
                ErrorCode::SshTimeout,
                format!("Connection to {}:{} timed out", target.host, target.port),
            )
        })??;

    let host_key = accepted.lock().ok().and_then(|g| g.clone());
    Ok(Connected { handle, host_key })
}
