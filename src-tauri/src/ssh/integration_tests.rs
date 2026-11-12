//! Integration tests against a real OpenSSH server.
//!
//! Skipped unless `DEVNEST_TEST_SSH_HOST` is set. See `scripts/test-ssh.sh`, which
//! starts a disposable `linuxserver/openssh-server` container and runs these tests.
//!
//! Env: DEVNEST_TEST_SSH_HOST, DEVNEST_TEST_SSH_PORT, DEVNEST_TEST_SSH_USER,
//!      DEVNEST_TEST_SSH_PASSWORD, DEVNEST_TEST_SSH_KEY (path), DEVNEST_TEST_SSH_KEY_PASSPHRASE

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;

use super::client::{self, Credentials, KeyMaterial, Target};
use super::manager::{SshManager, Status, StatusEvent};
use super::terminal::{TerminalEvent, TerminalSink};
use crate::database::known_hosts::KnownHost;
use crate::error::ErrorCode;
use crate::secret::Secret;

struct Env {
    target: Target,
    password: String,
    key_path: String,
    key_passphrase: String,
}

fn env() -> Option<Env> {
    let host = std::env::var("DEVNEST_TEST_SSH_HOST").ok()?;
    Some(Env {
        target: Target {
            host,
            port: std::env::var("DEVNEST_TEST_SSH_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(2222),
            username: std::env::var("DEVNEST_TEST_SSH_USER").unwrap_or_else(|_| "dev".into()),
            options: Default::default(),
        },
        password: std::env::var("DEVNEST_TEST_SSH_PASSWORD").unwrap_or_default(),
        key_path: std::env::var("DEVNEST_TEST_SSH_KEY").unwrap_or_default(),
        key_passphrase: std::env::var("DEVNEST_TEST_SSH_KEY_PASSPHRASE").unwrap_or_default(),
    })
}

fn password(env: &Env) -> Credentials {
    Credentials::Password(Secret::new(env.password.clone()))
}

/// Learn the server key through the TOFU error, like the UI does.
async fn trusted(env: &Env) -> Vec<KnownHost> {
    let err = client::connect(&env.target, &password(env), vec![], None)
        .await
        .err()
        .expect("unknown host must be rejected");
    assert_eq!(err.code, ErrorCode::HostKeyUnknown);
    let d = err.details.expect("host key details");
    vec![KnownHost {
        host: env.target.host.clone(),
        port: env.target.port,
        key_type: d["key_type"].as_str().unwrap().to_string(),
        fingerprint: d["fingerprint"].as_str().unwrap().to_string(),
    }]
}

#[tokio::test]
async fn host_key_and_password_auth() {
    let Some(env) = env() else { return };
    let known = trusted(&env).await;
    assert!(known[0].fingerprint.starts_with("SHA256:"));

    // Correct password.
    let c = client::connect(&env.target, &password(&env), known.clone(), None)
        .await
        .expect("password auth");
    assert!(c.host_key.is_some());
    let _ = c
        .handle
        .disconnect(russh::Disconnect::ByApplication, "", "en")
        .await;

    // Wrong password.
    let err = client::connect(
        &env.target,
        &Credentials::Password(Secret::new("definitely-wrong")),
        known.clone(),
        None,
    )
    .await
    .err()
    .unwrap();
    assert_eq!(err.code, ErrorCode::SshAuthFailed);

    // Changed host key.
    let mut tampered = known.clone();
    tampered[0].fingerprint = "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into();
    let err = client::connect(&env.target, &password(&env), tampered, None)
        .await
        .err()
        .unwrap();
    assert_eq!(err.code, ErrorCode::HostKeyChanged);
    assert!(err.details.unwrap()["previous_fingerprint"].is_string());

    // Unreachable port.
    let bad = Target {
        port: 1,
        ..env.target.clone()
    };
    let err = client::connect(&bad, &password(&env), known, None)
        .await
        .err()
        .unwrap();
    assert!(matches!(
        err.code,
        ErrorCode::SshConnect | ErrorCode::SshTimeout
    ));
}

#[tokio::test]
async fn key_auth_with_passphrase() {
    let Some(env) = env() else { return };
    if env.key_path.is_empty() {
        return;
    }
    let known = trusted(&env).await;
    let key = |pp: Option<&str>| Credentials::Key {
        key: KeyMaterial::Path(env.key_path.clone()),
        passphrase: pp.map(Secret::new),
    };

    let err = client::connect(&env.target, &key(None), known.clone(), None)
        .await
        .err()
        .unwrap();
    assert_eq!(err.code, ErrorCode::KeyPassphraseRequired);

    let err = client::connect(&env.target, &key(Some("wrong")), known.clone(), None)
        .await
        .err()
        .unwrap();
    assert_eq!(err.code, ErrorCode::KeyPassphraseRequired);
    assert_eq!(err.details.unwrap()["incorrect"], true);

    let c = client::connect(
        &env.target,
        &key(Some(&env.key_passphrase)),
        known.clone(),
        None,
    )
    .await
    .expect("key auth");
    let _ = c
        .handle
        .disconnect(russh::Disconnect::ByApplication, "", "en")
        .await;

    // Same key, stored as content (vault mode).
    let content = std::fs::read_to_string(&env.key_path).unwrap();
    let creds = Credentials::Key {
        key: KeyMaterial::Content(Secret::new(content)),
        passphrase: Some(Secret::new(env.key_passphrase.clone())),
    };
    let c = client::connect(&env.target, &creds, known.clone(), None)
        .await
        .expect("key content auth");
    let _ = c
        .handle
        .disconnect(russh::Disconnect::ByApplication, "", "en")
        .await;

    let err = client::connect(
        &env.target,
        &Credentials::Key {
            key: KeyMaterial::Path("/no/such/key".into()),
            passphrase: None,
        },
        known,
        None,
    )
    .await
    .err()
    .unwrap();
    assert_eq!(err.code, ErrorCode::KeyFileNotFound);
}

enum Out {
    Data(Vec<u8>),
    Event(TerminalEvent),
}

struct TestSink(mpsc::UnboundedSender<Out>);

impl TerminalSink for TestSink {
    fn data(&self, bytes: Vec<u8>) -> bool {
        self.0.send(Out::Data(bytes)).is_ok()
    }
    fn event(&self, event: TerminalEvent) -> bool {
        self.0.send(Out::Event(event)).is_ok()
    }
}

/// Read output until `needle` appears (or timeout). Returns accumulated text.
async fn read_until(rx: &mut mpsc::UnboundedReceiver<Out>, needle: &str) -> String {
    let mut acc = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !acc.contains(needle) {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Some(Out::Data(d))) => acc.push_str(&String::from_utf8_lossy(&d)),
            Ok(Some(Out::Event(_))) => {}
            _ => panic!("timed out waiting for {needle:?}; got: {acc}"),
        }
    }
    acc
}

#[tokio::test]
async fn terminal_lifecycle_through_manager() {
    let Some(env) = env() else { return };
    let known = trusted(&env).await;
    let events: Arc<Mutex<Vec<StatusEvent>>> = Arc::default();
    let ev2 = events.clone();
    let manager = SshManager::new(Arc::new(move |e| ev2.lock().unwrap().push(e)));

    let pw = env.password.clone();
    let (live, fresh) = manager
        .ensure_connected("conn-1", &env.target, known.clone(), || {
            Ok(Credentials::Password(Secret::new(pw)))
        })
        .await
        .expect("connect");
    assert!(fresh);

    // Two terminals share one connection.
    let (tx1, mut rx1) = mpsc::unbounded_channel();
    manager
        .open_terminal(
            "t1".into(),
            &live,
            "conn-1",
            80,
            24,
            Arc::new(TestSink(tx1)),
        )
        .await
        .unwrap();
    let (live2, fresh2) = manager
        .ensure_connected("conn-1", &env.target, known, || {
            panic!("must reuse the live connection")
        })
        .await
        .unwrap();
    assert!(!fresh2);
    let (tx2, mut rx2) = mpsc::unbounded_channel();
    manager
        .open_terminal(
            "t2".into(),
            &live2,
            "conn-1",
            80,
            24,
            Arc::new(TestSink(tx2)),
        )
        .await
        .unwrap();
    assert_eq!(manager.sessions().unwrap()[0].terminals, 2);

    // Interactive I/O.
    manager
        .write("t1", b"echo dev$((40+2))nest\n")
        .await
        .unwrap();
    read_until(&mut rx1, "dev42nest").await;

    // Resize is propagated to the PTY.
    manager.resize("t1", 132, 43).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    manager.write("t1", b"stty size\n").await.unwrap();
    read_until(&mut rx1, "43 132").await;

    // Closing one terminal keeps the connection for the other.
    manager.close_terminal("t2").await.unwrap();
    assert_eq!(manager.sessions().unwrap().len(), 1);
    drop(rx2.try_recv());

    // `exit` ends the shell -> Exit + Closed events -> connection released.
    manager.write("t1", b"exit 7\n").await.unwrap();
    let mut exit_code = None;
    let mut closed = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !closed {
        match tokio::time::timeout_at(deadline, rx1.recv()).await {
            Ok(Some(Out::Event(TerminalEvent::Exit { code, .. }))) => exit_code = code,
            Ok(Some(Out::Event(TerminalEvent::Closed { .. }))) => closed = true,
            Ok(Some(_)) => {}
            _ => panic!("terminal did not close"),
        }
    }
    assert_eq!(exit_code, Some(7));
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        manager.sessions().unwrap().is_empty(),
        "connection should be released"
    );
    assert!(manager.write("t1", b"x").await.is_err());

    let statuses: Vec<Status> = events.lock().unwrap().iter().map(|e| e.status).collect();
    assert_eq!(statuses.first(), Some(&Status::Connecting));
    assert!(statuses.contains(&Status::Connected));
    assert_eq!(statuses.last(), Some(&Status::Disconnected));
}

#[tokio::test]
async fn host_key_algorithm_options() {
    let Some(env) = env() else { return };
    let mut known = trusted(&env).await; // ed25519 by default

    // Force an RSA host key (SHA-2 signature): a new key type -> TOFU prompt, not "changed".
    let rsa = Target {
        options: super::options::SshOptions::parse("HostKeyAlgorithms=rsa-sha2-512,rsa-sha2-256")
            .unwrap(),
        ..env.target.clone()
    };
    let err = client::connect(&rsa, &password(&env), known.clone(), None)
        .await
        .err()
        .unwrap();
    assert_eq!(err.code, ErrorCode::HostKeyUnknown, "{err:?}");
    let d = err.details.unwrap();
    assert_eq!(d["key_type"], "ssh-rsa");
    known.push(KnownHost {
        host: env.target.host.clone(),
        port: env.target.port,
        key_type: "ssh-rsa".into(),
        fingerprint: d["fingerprint"].as_str().unwrap().into(),
    });
    let c = client::connect(&rsa, &password(&env), known.clone(), None)
        .await
        .expect("rsa host key");
    let _ = c
        .handle
        .disconnect(russh::Disconnect::ByApplication, "", "en")
        .await;

    // Appending legacy algorithms keeps the modern defaults working.
    let legacy = Target {
        options: super::options::SshOptions::parse(
            "HostKeyAlgorithms=+ssh-rsa
PubkeyAcceptedKeyTypes=+ssh-rsa
KexAlgorithms=+diffie-hellman-group14-sha1",
        )
        .unwrap(),
        ..env.target.clone()
    };
    let c = client::connect(&legacy, &password(&env), known.clone(), None)
        .await
        .expect("append options");
    let _ = c
        .handle
        .disconnect(russh::Disconnect::ByApplication, "", "en")
        .await;

    // A cipher the server does not offer -> friendly "no common algorithm" error.
    let bad = Target {
        options: super::options::SshOptions::parse("Ciphers=aes128-cbc").unwrap(),
        ..env.target.clone()
    };
    let err = client::connect(&bad, &password(&env), known, None)
        .await
        .err()
        .unwrap();
    assert_eq!(err.code, ErrorCode::SshConnect, "{err:?}");
    assert!(err.message.contains("Ciphers"), "{}", err.message);
}

#[tokio::test]
async fn sftp_browsing_download_and_overwrite() {
    let Some(env) = env() else { return };
    let known = trusted(&env).await;
    let manager = SshManager::new(Arc::new(|_| {}));
    let connection_id = "sftp-integration";
    manager
        .ensure_connected(connection_id, &env.target, known, || Ok(password(&env)))
        .await
        .expect("SSH connection");

    let remote = manager
        .list_remote_dir(connection_id, "/etc")
        .await
        .expect("SFTP directory listing");
    assert!(remote.entries.iter().any(|entry| entry.name == "hostname"));

    let local = tempfile::tempdir().unwrap();
    let downloaded = manager
        .download_file(
            connection_id,
            "/etc/hostname",
            local.path(),
            "hostname",
            false,
        )
        .await
        .expect("SFTP download");
    let content = std::fs::read(&downloaded).unwrap();
    assert!(!content.is_empty());

    let error = manager
        .download_file(
            connection_id,
            "/etc/hostname",
            local.path(),
            "hostname",
            false,
        )
        .await
        .expect_err("existing files must not be overwritten without confirmation");
    assert_eq!(error.code, ErrorCode::FileExists);

    manager
        .download_file(
            connection_id,
            "/etc/hostname",
            local.path(),
            "hostname",
            true,
        )
        .await
        .expect("confirmed overwrite");
    assert_eq!(std::fs::read(&downloaded).unwrap(), content);
    manager.disconnect(connection_id).await.unwrap();
}
