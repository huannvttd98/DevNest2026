//! Vault: envelope encryption for secrets.
//!
//! * A random 256-bit master key lives in the OS secure store
//!   (macOS Keychain / Windows Credential Manager) – one single entry.
//! * Each secret is encrypted with XChaCha20-Poly1305 using a random 24-byte nonce and
//!   `owner_id || ":" || kind` as associated data (so ciphertexts cannot be swapped
//!   between rows) and stored in the `secrets` table.
//! * If the master key disappears (OS reset, DB copied to another machine) old secrets
//!   simply fail to decrypt; callers treat them as "not stored" and prompt the user.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use crate::database::secrets::{EncryptedSecret, SecretKind};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::secret::Secret;

const KEYRING_SERVICE: &str = "com.devnest.app";
const KEYRING_ACCOUNT: &str = "vault-master-key";
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;

/// Where the master key is persisted.
pub trait MasterKeyStore: Send + Sync {
    fn load(&self) -> AppResult<Option<Zeroizing<Vec<u8>>>>;
    fn save(&self, key: &[u8]) -> AppResult<()>;
}

/// OS keyring backed store.
pub struct KeyringStore;

impl MasterKeyStore for KeyringStore {
    fn load(&self) -> AppResult<Option<Zeroizing<Vec<u8>>>> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).map_err(vault_err)?;
        match entry.get_secret() {
            Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(vault_err(e)),
        }
    }

    fn save(&self, key: &[u8]) -> AppResult<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).map_err(vault_err)?;
        entry.set_secret(key).map_err(vault_err)
    }
}

fn vault_err(e: keyring::Error) -> AppError {
    AppError::new(ErrorCode::Vault, format!("OS secure storage error: {e}"))
}

/// In-memory store, for tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore(std::sync::Mutex<Option<Vec<u8>>>);

#[cfg(test)]
impl MasterKeyStore for MemoryStore {
    fn load(&self) -> AppResult<Option<Zeroizing<Vec<u8>>>> {
        Ok(self.0.lock().unwrap().clone().map(Zeroizing::new))
    }
    fn save(&self, key: &[u8]) -> AppResult<()> {
        *self.0.lock().unwrap() = Some(key.to_vec());
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultState {
    /// Master key loaded from the OS store.
    Ready,
    /// No key existed; a new one was created (first run or key lost).
    Created,
}

pub struct Vault {
    cipher: XChaCha20Poly1305,
    pub state: VaultState,
}

impl Vault {
    pub fn open(store: &dyn MasterKeyStore) -> AppResult<Self> {
        let (key, state) = match store.load()? {
            Some(k) if k.len() == KEY_LEN => (k, VaultState::Ready),
            Some(_) => {
                tracing::warn!("vault master key has invalid length; generating a new one");
                (Self::create_key(store)?, VaultState::Created)
            }
            None => (Self::create_key(store)?, VaultState::Created),
        };
        let cipher = XChaCha20Poly1305::new_from_slice(&key)
            .map_err(|_| AppError::new(ErrorCode::Vault, "Invalid vault key"))?;
        Ok(Self { cipher, state })
    }

    fn create_key(store: &dyn MasterKeyStore) -> AppResult<Zeroizing<Vec<u8>>> {
        let mut key = Zeroizing::new(vec![0u8; KEY_LEN]);
        getrandom::fill(&mut key)
            .map_err(|e| AppError::new(ErrorCode::Vault, format!("RNG failure: {e}")))?;
        store.save(&key)?;
        tracing::info!("vault master key created");
        Ok(key)
    }

    fn aad(owner_id: &str, kind: SecretKind) -> Vec<u8> {
        format!("{owner_id}:{}", kind.as_str()).into_bytes()
    }

    pub fn encrypt(
        &self,
        owner_id: &str,
        kind: SecretKind,
        plaintext: &Secret,
    ) -> AppResult<EncryptedSecret> {
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce)
            .map_err(|e| AppError::new(ErrorCode::Vault, format!("RNG failure: {e}")))?;
        let aad = Self::aad(owner_id, kind);
        let ciphertext = self
            .cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext.expose().as_bytes(),
                    aad: &aad,
                },
            )
            .map_err(|_| AppError::new(ErrorCode::Vault, "Encryption failed"))?;
        Ok(EncryptedSecret {
            nonce: nonce.to_vec(),
            ciphertext,
        })
    }

    /// Returns `None` when the secret cannot be decrypted (e.g. master key changed).
    pub fn decrypt(&self, owner_id: &str, kind: SecretKind, s: &EncryptedSecret) -> Option<Secret> {
        let nonce: [u8; NONCE_LEN] = s.nonce.as_slice().try_into().ok()?;
        let aad = Self::aad(owner_id, kind);
        let plain = Zeroizing::new(
            self.cipher
                .decrypt(
                    &XNonce::from(nonce),
                    Payload {
                        msg: &s.ciphertext,
                        aad: &aad,
                    },
                )
                .ok()?,
        );
        let text = std::str::from_utf8(&plain).ok()?;
        Some(Secret::new(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_creation() {
        let store = MemoryStore::default();
        let v = Vault::open(&store).unwrap();
        assert_eq!(v.state, VaultState::Created);
        let enc = v
            .encrypt("c1", SecretKind::Password, &Secret::new("p@ss"))
            .unwrap();
        assert_ne!(enc.ciphertext, b"p@ss");
        let dec = v.decrypt("c1", SecretKind::Password, &enc).unwrap();
        assert_eq!(dec.expose(), "p@ss");

        // Re-open uses the same stored key.
        let v2 = Vault::open(&store).unwrap();
        assert_eq!(v2.state, VaultState::Ready);
        assert_eq!(
            v2.decrypt("c1", SecretKind::Password, &enc)
                .unwrap()
                .expose(),
            "p@ss"
        );
    }

    #[test]
    fn aad_binds_owner_and_kind() {
        let store = MemoryStore::default();
        let v = Vault::open(&store).unwrap();
        let enc = v
            .encrypt("c1", SecretKind::Password, &Secret::new("x"))
            .unwrap();
        assert!(v.decrypt("c2", SecretKind::Password, &enc).is_none());
        assert!(v.decrypt("c1", SecretKind::Passphrase, &enc).is_none());
    }

    #[test]
    fn lost_key_yields_none() {
        let v1 = Vault::open(&MemoryStore::default()).unwrap();
        let enc = v1
            .encrypt("c1", SecretKind::Password, &Secret::new("x"))
            .unwrap();
        let v2 = Vault::open(&MemoryStore::default()).unwrap();
        assert!(v2.decrypt("c1", SecretKind::Password, &enc).is_none());
    }

    #[test]
    fn nonces_are_unique() {
        let v = Vault::open(&MemoryStore::default()).unwrap();
        let a = v
            .encrypt("c", SecretKind::Password, &Secret::new("same"))
            .unwrap();
        let b = v
            .encrypt("c", SecretKind::Password, &Secret::new("same"))
            .unwrap();
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.ciphertext, b.ciphertext);
    }
}
