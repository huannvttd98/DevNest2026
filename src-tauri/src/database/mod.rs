//! SQLite storage (local only).
//!
//! * One connection guarded by a mutex – the data volume is tiny and every query is
//!   short, so this is simpler and safer than a pool.
//! * Schema is managed by `rusqlite_migration` (`PRAGMA user_version`).
//! * Before applying pending migrations to an existing database, a copy is written to
//!   `backups/` with the SQLite Online Backup API (the newest 5 are kept).

pub mod connections;
pub mod groups;
pub mod known_hosts;
pub mod migrations;
pub mod secrets;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

const BACKUPS_TO_KEEP: usize = 5;

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// Open (or create) the database at `path` and bring the schema up to date.
    pub fn open(path: &Path, backups_dir: &Path) -> AppResult<Self> {
        let existed = path.exists();
        let mut conn = Connection::open(path)?;
        configure(&conn)?;

        let migrations = migrations::migrations();
        let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let current = usize::try_from(current).unwrap_or(0);
        let latest = migrations::LATEST_VERSION;
        if existed && current < latest {
            backup(&conn, backups_dir, current)?;
        }
        migrations.to_latest(&mut conn)?;
        tracing::info!(from = current, to = latest, "database ready");

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// In-memory database for tests.
    #[cfg(test)]
    pub fn open_in_memory() -> AppResult<Self> {
        let mut conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::migrations().to_latest(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn conn(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| AppError::internal("database lock poisoned"))
    }
}

fn configure(conn: &Connection) -> AppResult<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

fn backup(conn: &Connection, backups_dir: &Path, version: usize) -> AppResult<PathBuf> {
    std::fs::create_dir_all(backups_dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let target = backups_dir.join(format!("devnest-v{version}-{stamp}.db"));
    conn.backup(rusqlite::MAIN_DB, &target, None)?;
    tracing::info!(path = %target.display(), "database backed up before migration");
    prune_backups(backups_dir)?;
    Ok(target)
}

fn prune_backups(dir: &Path) -> AppResult<()> {
    let mut files: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("devnest-") && name.ends_with(".db")
        })
        .collect();
    // File names embed a sortable timestamp; newest last.
    files.sort_by_key(|e| e.file_name());
    if files.len() > BACKUPS_TO_KEEP {
        for old in &files[..files.len() - BACKUPS_TO_KEEP] {
            let _ = std::fs::remove_file(old.path());
        }
    }
    Ok(())
}

/// Current UTC time as RFC 3339 string (stored as TEXT).
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// New time-ordered UUID v7 as string.
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        migrations::migrations().validate().unwrap();
    }

    #[test]
    fn open_creates_and_reopen_backs_up_only_when_needed() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("devnest.db");
        let backups = dir.path().join("backups");
        {
            let _db = Db::open(&db_path, &backups).unwrap();
        }
        // Re-open at the same version: no backup expected.
        {
            let _db = Db::open(&db_path, &backups).unwrap();
        }
        assert!(!backups.exists() || std::fs::read_dir(&backups).unwrap().count() == 0);
    }

    #[test]
    fn prune_keeps_newest_five() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..8 {
            std::fs::write(
                dir.path().join(format!("devnest-v1-2026010{i}-000000.db")),
                b"x",
            )
            .unwrap();
        }
        prune_backups(dir.path()).unwrap();
        let mut names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(names.len(), 5);
        assert!(names[0].contains("20260103"));
    }
}
