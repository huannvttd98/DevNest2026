//! Raw storage of encrypted secrets. Encryption/decryption lives in `vault`.

use rusqlite::{Connection, OptionalExtension, params};

use super::now;
use crate::error::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    Password,
    Passphrase,
    PrivateKey,
}

impl SecretKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SecretKind::Password => "password",
            SecretKind::Passphrase => "passphrase",
            SecretKind::PrivateKey => "private_key",
        }
    }
}

pub struct EncryptedSecret {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

pub fn put(
    conn: &Connection,
    owner_id: &str,
    kind: SecretKind,
    s: &EncryptedSecret,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO secrets (owner_id, kind, nonce, ciphertext, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(owner_id, kind) DO UPDATE SET
            nonce = excluded.nonce, ciphertext = excluded.ciphertext, created_at = excluded.created_at",
        params![owner_id, kind.as_str(), s.nonce, s.ciphertext, now()],
    )?;
    Ok(())
}

pub fn get(
    conn: &Connection,
    owner_id: &str,
    kind: SecretKind,
) -> AppResult<Option<EncryptedSecret>> {
    Ok(conn
        .query_row(
            "SELECT nonce, ciphertext FROM secrets WHERE owner_id = ?1 AND kind = ?2",
            params![owner_id, kind.as_str()],
            |r| {
                Ok(EncryptedSecret {
                    nonce: r.get(0)?,
                    ciphertext: r.get(1)?,
                })
            },
        )
        .optional()?)
}

pub fn delete(conn: &Connection, owner_id: &str, kind: SecretKind) -> AppResult<()> {
    conn.execute(
        "DELETE FROM secrets WHERE owner_id = ?1 AND kind = ?2",
        params![owner_id, kind.as_str()],
    )?;
    Ok(())
}

pub fn delete_all(conn: &Connection, owner_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM secrets WHERE owner_id = ?1", [owner_id])?;
    Ok(())
}
