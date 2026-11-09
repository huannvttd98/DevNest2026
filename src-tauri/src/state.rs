use std::path::PathBuf;

use crate::database::Db;
use crate::error::{AppError, AppResult};
use crate::ssh::SshManager;
use crate::vault::Vault;

pub struct AppState {
    pub db: Db,
    vault: Result<Vault, AppError>,
    pub ssh: SshManager,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl AppState {
    pub fn new(
        db: Db,
        vault: Result<Vault, AppError>,
        ssh: SshManager,
        data_dir: PathBuf,
        log_dir: PathBuf,
    ) -> Self {
        Self {
            db,
            vault,
            ssh,
            data_dir,
            log_dir,
        }
    }

    /// The vault, or the error that prevented opening it (OS keyring unavailable).
    pub fn vault(&self) -> AppResult<&Vault> {
        self.vault.as_ref().map_err(Clone::clone)
    }

    pub fn vault_status(&self) -> Result<crate::vault::VaultState, String> {
        self.vault
            .as_ref()
            .map(|v| v.state)
            .map_err(|e| e.message.clone())
    }
}
