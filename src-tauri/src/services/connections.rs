//! Connection use-cases: save connections together with their secrets, and resolve
//! the credentials needed to connect.

use serde::Deserialize;

use crate::database::connections::{
    self as repo, AuthType, ConnectionFields, ConnectionRecord, KeySource,
};
use crate::database::secrets::{self, SecretKind};
use crate::database::{Db, known_hosts};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::secret::Secret;
use crate::ssh::client::{self, Credentials, KeyMaterial, Target};
use crate::ssh::options::SshOptions;
use crate::vault::Vault;

/// What to do with a stored secret when saving the form.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SecretUpdate {
    /// Leave the stored value untouched (the form shows "saved").
    #[default]
    Keep,
    Set {
        value: Secret,
    },
    Clear,
    /// Private key only: read the key file from disk (backend side) and store its content.
    ImportFile {
        path: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveConnectionInput {
    #[serde(flatten)]
    pub fields: ConnectionFields,
    #[serde(default)]
    pub password: SecretUpdate,
    #[serde(default)]
    pub passphrase: SecretUpdate,
    #[serde(default)]
    pub private_key: SecretUpdate,
}

/// One-off secrets typed in a prompt; take precedence over stored ones.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CredentialOverrides {
    pub password: Option<Secret>,
    pub passphrase: Option<Secret>,
}

fn resolve_value(update: &SecretUpdate, kind: SecretKind) -> AppResult<Option<Option<Secret>>> {
    // Some(Some(v)) = set, Some(None) = clear, None = keep
    Ok(match update {
        SecretUpdate::Keep => None,
        SecretUpdate::Clear => Some(None),
        SecretUpdate::Set { value } if value.is_empty() => Some(None),
        SecretUpdate::Set { value } => Some(Some(value.clone())),
        SecretUpdate::ImportFile { path } => {
            if kind != SecretKind::PrivateKey {
                return Err(AppError::validation(
                    "File import is only supported for private keys",
                ));
            }
            let content = client::read_key_file(path.trim())?;
            Some(Some(Secret::new(content.as_str())))
        }
    })
}

/// Create (id = None) or update a connection and its secrets atomically.
pub fn save(
    db: &Db,
    vault: &Vault,
    id: Option<&str>,
    input: &SaveConnectionInput,
) -> AppResult<ConnectionRecord> {
    let password = resolve_value(&input.password, SecretKind::Password)?;
    let passphrase = resolve_value(&input.passphrase, SecretKind::Passphrase)?;
    let private_key = resolve_value(&input.private_key, SecretKind::PrivateKey)?;

    // Validate key content early so the user gets feedback in the form.
    if let Some(Some(key)) = &private_key {
        let pp = match &passphrase {
            Some(Some(p)) => Some(p),
            _ => None,
        };
        client::validate_key_content(key, pp)?;
    }

    let mut conn = db.conn()?;
    let fields = repo::validate(&conn, &input.fields)?;
    let tx = conn.transaction()?;
    let id = match id {
        Some(id) => {
            repo::update(&tx, id, &fields)?;
            id.to_string()
        }
        None => repo::insert(&tx, &fields)?,
    };

    let apply = |kind: SecretKind, value: &Option<Option<Secret>>| -> AppResult<()> {
        match value {
            None => Ok(()),
            Some(None) => secrets::delete(&tx, &id, kind),
            Some(Some(v)) => secrets::put(&tx, &id, kind, &vault.encrypt(&id, kind, v)?),
        }
    };
    apply(SecretKind::Password, &password)?;
    apply(SecretKind::Passphrase, &passphrase)?;
    apply(SecretKind::PrivateKey, &private_key)?;

    // Drop secrets that no longer apply to the chosen auth method.
    match fields.auth_type {
        AuthType::Password => {
            secrets::delete(&tx, &id, SecretKind::Passphrase)?;
            secrets::delete(&tx, &id, SecretKind::PrivateKey)?;
        }
        AuthType::Key => {
            secrets::delete(&tx, &id, SecretKind::Password)?;
            if fields.key_source == Some(KeySource::Path) {
                secrets::delete(&tx, &id, SecretKind::PrivateKey)?;
            }
        }
    }

    if fields.auth_type == AuthType::Key
        && fields.key_source == Some(KeySource::Vault)
        && secrets::get(&tx, &id, SecretKind::PrivateKey)?.is_none()
    {
        return Err(AppError::validation(
            "Paste or import a private key to store it in the vault",
        ));
    }

    tx.commit()?;
    repo::get(&conn, &id)
}

pub fn delete(db: &Db, id: &str) -> AppResult<()> {
    let mut conn = db.conn()?;
    let tx = conn.transaction()?;
    secrets::delete_all(&tx, id)?;
    repo::delete(&tx, id)?;
    tx.commit()?;
    Ok(())
}

fn stored_secret(db: &Db, vault: &Vault, id: &str, kind: SecretKind) -> AppResult<Option<Secret>> {
    let conn = db.conn()?;
    let Some(enc) = secrets::get(&conn, id, kind)? else {
        return Ok(None);
    };
    let dec = vault.decrypt(id, kind, &enc);
    if dec.is_none() {
        tracing::warn!(
            connection_id = id,
            kind = kind.as_str(),
            "stored secret could not be decrypted"
        );
    }
    Ok(dec)
}

pub fn target_of(rec: &ConnectionRecord) -> AppResult<Target> {
    Ok(Target {
        host: rec.host.clone(),
        port: rec.port,
        username: rec.username.clone(),
        options: SshOptions::parse(&rec.ssh_options)?,
    })
}

pub fn known_hosts_for(db: &Db, rec: &ConnectionRecord) -> AppResult<Vec<known_hosts::KnownHost>> {
    let conn = db.conn()?;
    known_hosts::for_host(&conn, &rec.host, rec.port)
}

/// Build the credentials for a saved connection (stored secrets + prompt overrides).
pub fn credentials_for(
    db: &Db,
    vault: &Vault,
    rec: &ConnectionRecord,
    overrides: &CredentialOverrides,
) -> AppResult<Credentials> {
    match rec.auth_type {
        AuthType::Password => {
            let pw = match overrides.password.clone().filter(|p| !p.is_empty()) {
                Some(p) => Some(p),
                None => stored_secret(db, vault, &rec.id, SecretKind::Password)?,
            };
            pw.map(Credentials::Password).ok_or_else(|| {
                AppError::new(ErrorCode::AuthPasswordRequired, "Password is required")
            })
        }
        AuthType::Key => {
            let key = match rec.key_source.unwrap_or(KeySource::Path) {
                KeySource::Path => KeyMaterial::Path(rec.key_path.clone().unwrap_or_default()),
                KeySource::Vault => KeyMaterial::Content(
                    stored_secret(db, vault, &rec.id, SecretKind::PrivateKey)?.ok_or_else(|| {
                        AppError::new(
                            ErrorCode::KeyInvalid,
                            "No private key stored for this connection (or it cannot be decrypted). Edit the connection and import the key again.",
                        )
                    })?,
                ),
            };
            let passphrase = match overrides.passphrase.clone().filter(|p| !p.is_empty()) {
                Some(p) => Some(p),
                None => stored_secret(db, vault, &rec.id, SecretKind::Passphrase)?,
            };
            Ok(Credentials::Key { key, passphrase })
        }
    }
}

/// Target + credentials for "Test connection" from (possibly unsaved) form data.
/// Secrets marked `Keep` are taken from the stored connection `id`, if any.
pub fn test_params(
    db: &Db,
    vault: &Vault,
    id: Option<&str>,
    input: &SaveConnectionInput,
    overrides: &CredentialOverrides,
) -> AppResult<(Target, Credentials)> {
    let fields = repo::validate(&*db.conn()?, &input.fields)?;
    let target = Target {
        host: fields.host.clone(),
        port: fields.port,
        username: fields.username.clone(),
        options: SshOptions::parse(&fields.ssh_options)?,
    };
    let pick = |update: &SecretUpdate,
                kind: SecretKind,
                ov: Option<&Secret>|
     -> AppResult<Option<Secret>> {
        if let Some(v) = ov.filter(|v| !v.is_empty()) {
            return Ok(Some(v.clone()));
        }
        match resolve_value(update, kind)? {
            Some(v) => Ok(v),
            None => match id {
                Some(id) => stored_secret(db, vault, id, kind),
                None => Ok(None),
            },
        }
    };
    let creds = match fields.auth_type {
        AuthType::Password => Credentials::Password(
            pick(
                &input.password,
                SecretKind::Password,
                overrides.password.as_ref(),
            )?
            .ok_or_else(|| {
                AppError::new(ErrorCode::AuthPasswordRequired, "Password is required")
            })?,
        ),
        AuthType::Key => {
            let key = match fields.key_source.unwrap_or(KeySource::Path) {
                KeySource::Path => KeyMaterial::Path(fields.key_path.clone().unwrap_or_default()),
                KeySource::Vault => KeyMaterial::Content(
                    pick(&input.private_key, SecretKind::PrivateKey, None)?.ok_or_else(|| {
                        AppError::new(ErrorCode::KeyInvalid, "No private key provided")
                    })?,
                ),
            };
            let passphrase = pick(
                &input.passphrase,
                SecretKind::Passphrase,
                overrides.passphrase.as_ref(),
            )?;
            Credentials::Key { key, passphrase }
        }
    };
    Ok((target, creds))
}

/// Persist a secret typed in a prompt (when the user ticked "save").
pub fn store_prompted(
    db: &Db,
    vault: &Vault,
    id: &str,
    overrides: &CredentialOverrides,
) -> AppResult<()> {
    let conn = db.conn()?;
    if let Some(p) = overrides.password.as_ref().filter(|p| !p.is_empty()) {
        secrets::put(
            &conn,
            id,
            SecretKind::Password,
            &vault.encrypt(id, SecretKind::Password, p)?,
        )?;
    }
    if let Some(p) = overrides.passphrase.as_ref().filter(|p| !p.is_empty()) {
        secrets::put(
            &conn,
            id,
            SecretKind::Passphrase,
            &vault.encrypt(id, SecretKind::Passphrase, p)?,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::MemoryStore;

    fn input(auth: AuthType) -> SaveConnectionInput {
        SaveConnectionInput {
            fields: ConnectionFields {
                group_id: None,
                name: "srv".into(),
                host: "example.com".into(),
                port: 22,
                username: "root".into(),
                auth_type: auth,
                key_source: (auth == AuthType::Key).then_some(KeySource::Path),
                key_path: (auth == AuthType::Key).then(|| "/tmp/id".into()),
                ssh_options: None,
            },
            password: SecretUpdate::Keep,
            passphrase: SecretUpdate::Keep,
            private_key: SecretUpdate::Keep,
        }
    }

    #[test]
    fn password_lifecycle() {
        let db = Db::open_in_memory().unwrap();
        let vault = Vault::open(&MemoryStore::default()).unwrap();
        let mut inp = input(AuthType::Password);
        inp.password = SecretUpdate::Set {
            value: Secret::new("s3cret"),
        };
        let rec = save(&db, &vault, None, &inp).unwrap();
        assert!(rec.has_password);

        // Stored ciphertext must not contain the plaintext.
        {
            let conn = db.conn().unwrap();
            let enc = secrets::get(&conn, &rec.id, SecretKind::Password)
                .unwrap()
                .unwrap();
            assert!(!enc.ciphertext.windows(6).any(|w| w == b"s3cret"));
        }

        let creds = credentials_for(&db, &vault, &rec, &CredentialOverrides::default()).unwrap();
        match creds {
            Credentials::Password(p) => assert_eq!(p.expose(), "s3cret"),
            _ => panic!("expected password"),
        }

        // Keep leaves it, Clear removes it.
        inp.password = SecretUpdate::Keep;
        assert!(save(&db, &vault, Some(&rec.id), &inp).unwrap().has_password);
        inp.password = SecretUpdate::Clear;
        let rec = save(&db, &vault, Some(&rec.id), &inp).unwrap();
        assert!(!rec.has_password);
        let err = credentials_for(&db, &vault, &rec, &CredentialOverrides::default()).unwrap_err();
        assert_eq!(err.code, ErrorCode::AuthPasswordRequired);

        // Override wins.
        let ov = CredentialOverrides {
            password: Some(Secret::new("typed")),
            passphrase: None,
        };
        assert!(matches!(
            credentials_for(&db, &vault, &rec, &ov).unwrap(),
            Credentials::Password(_)
        ));
        store_prompted(&db, &vault, &rec.id, &ov).unwrap();
        assert!(
            repo::get(&db.conn().unwrap(), &rec.id)
                .unwrap()
                .has_password
        );

        delete(&db, &rec.id).unwrap();
        let conn = db.conn().unwrap();
        assert!(
            secrets::get(&conn, &rec.id, SecretKind::Password)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn switching_to_key_drops_password() {
        let db = Db::open_in_memory().unwrap();
        let vault = Vault::open(&MemoryStore::default()).unwrap();
        let mut inp = input(AuthType::Password);
        inp.password = SecretUpdate::Set {
            value: Secret::new("pw"),
        };
        let rec = save(&db, &vault, None, &inp).unwrap();
        let mut inp2 = input(AuthType::Key);
        inp2.passphrase = SecretUpdate::Set {
            value: Secret::new("pp"),
        };
        let rec = save(&db, &vault, Some(&rec.id), &inp2).unwrap();
        assert!(!rec.has_password);
        assert!(rec.has_passphrase);
    }

    #[test]
    fn vault_key_source_requires_key() {
        let db = Db::open_in_memory().unwrap();
        let vault = Vault::open(&MemoryStore::default()).unwrap();
        let mut inp = input(AuthType::Key);
        inp.fields.key_source = Some(KeySource::Vault);
        inp.fields.key_path = None;
        assert!(save(&db, &vault, None, &inp).is_err());
        inp.private_key = SecretUpdate::Set {
            value: Secret::new("not a key"),
        };
        let err = save(&db, &vault, None, &inp).unwrap_err();
        assert_eq!(err.code, ErrorCode::KeyInvalid);
        assert!(repo::list(&db.conn().unwrap()).unwrap().is_empty());
    }
}
