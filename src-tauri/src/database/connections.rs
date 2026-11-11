use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use super::{groups, new_id, now};
use crate::error::{AppError, AppResult};
use crate::ssh::options::{SshOptions, normalize as normalize_options};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
    Password,
    Key,
}

impl AuthType {
    pub fn as_str(self) -> &'static str {
        match self {
            AuthType::Password => "password",
            AuthType::Key => "key",
        }
    }
    fn parse(s: &str) -> Self {
        if s == "key" {
            AuthType::Key
        } else {
            AuthType::Password
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeySource {
    /// Reference a key file on disk.
    Path,
    /// Key content stored (encrypted) in the DevNest vault.
    Vault,
}

impl KeySource {
    pub fn as_str(self) -> &'static str {
        match self {
            KeySource::Path => "path",
            KeySource::Vault => "vault",
        }
    }
    fn parse(s: &str) -> Self {
        if s == "vault" {
            KeySource::Vault
        } else {
            KeySource::Path
        }
    }
}

/// Connection as exposed to the frontend. Contains NO secret values,
/// only flags telling whether a secret is stored.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionRecord {
    pub id: String,
    pub group_id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: AuthType,
    pub key_source: Option<KeySource>,
    pub key_path: Option<String>,
    /// OpenSSH-style `-o` algorithm options, one `Key=Value` per line.
    pub ssh_options: String,
    pub has_password: bool,
    pub has_passphrase: bool,
    pub has_private_key: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub last_connected_at: Option<String>,
}

/// Non-secret fields of a connection, as submitted by the form.
#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionFields {
    pub group_id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: AuthType,
    pub key_source: Option<KeySource>,
    pub key_path: Option<String>,
    #[serde(default)]
    pub ssh_options: Option<String>,
}

/// Validated, normalized fields.
#[derive(Debug, Clone)]
pub struct ValidFields {
    pub group_id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: AuthType,
    pub key_source: Option<KeySource>,
    pub key_path: Option<String>,
    pub ssh_options: String,
}

pub fn validate(conn: &Connection, f: &ConnectionFields) -> AppResult<ValidFields> {
    let name = f.name.trim();
    let host = f.host.trim();
    let username = f.username.trim();
    if name.is_empty() {
        return Err(AppError::validation("Connection name is required"));
    }
    if name.chars().count() > 100 {
        return Err(AppError::validation(
            "Connection name must be at most 100 characters",
        ));
    }
    validate_host(host)?;
    if f.port == 0 {
        return Err(AppError::validation("Port must be between 1 and 65535"));
    }
    if username.is_empty() {
        return Err(AppError::validation("Username is required"));
    }
    if username
        .chars()
        .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(AppError::validation("Username must not contain spaces"));
    }
    let group_id = match f.group_id.as_deref().map(str::trim) {
        Some(g) if !g.is_empty() => {
            if !groups::exists(conn, g)? {
                return Err(AppError::validation("Selected group does not exist"));
            }
            Some(g.to_string())
        }
        _ => None,
    };
    let (key_source, key_path) = match f.auth_type {
        AuthType::Password => (None, None),
        AuthType::Key => {
            let source = f
                .key_source
                .ok_or_else(|| AppError::validation("Choose where the private key comes from"))?;
            match source {
                KeySource::Path => {
                    let p = f.key_path.as_deref().map(str::trim).unwrap_or_default();
                    if p.is_empty() {
                        return Err(AppError::validation("Private key path is required"));
                    }
                    (Some(source), Some(p.to_string()))
                }
                KeySource::Vault => (Some(source), None),
            }
        }
    };
    let ssh_options = normalize_options(f.ssh_options.as_deref().unwrap_or_default());
    SshOptions::parse(&ssh_options)?;
    Ok(ValidFields {
        group_id,
        name: name.to_string(),
        host: host.to_string(),
        port: f.port,
        username: username.to_string(),
        auth_type: f.auth_type,
        key_source,
        key_path,
        ssh_options,
    })
}

fn validate_host(host: &str) -> AppResult<()> {
    if host.is_empty() {
        return Err(AppError::validation("Hostname / IP is required"));
    }
    if host.len() > 253 {
        return Err(AppError::validation("Hostname is too long"));
    }
    // IPv6 literal (optionally in brackets) or hostname / IPv4.
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if bare.parse::<std::net::IpAddr>().is_ok() {
        return Ok(());
    }
    let ok = host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_');
    if !ok {
        return Err(AppError::validation("Hostname contains invalid characters"));
    }
    Ok(())
}

const SELECT: &str = "
SELECT c.id, c.group_id, c.name, c.host, c.port, c.username, c.auth_type, c.key_source,
       c.key_path, c.sort_order, c.created_at, c.updated_at, c.last_connected_at, c.ssh_options,
       EXISTS(SELECT 1 FROM secrets s WHERE s.owner_id = c.id AND s.kind = 'password'),
       EXISTS(SELECT 1 FROM secrets s WHERE s.owner_id = c.id AND s.kind = 'passphrase'),
       EXISTS(SELECT 1 FROM secrets s WHERE s.owner_id = c.id AND s.kind = 'private_key')
FROM connections c";

fn map_row(r: &Row<'_>) -> rusqlite::Result<ConnectionRecord> {
    let auth: String = r.get(6)?;
    let key_source: Option<String> = r.get(7)?;
    let port: i64 = r.get(4)?;
    Ok(ConnectionRecord {
        id: r.get(0)?,
        group_id: r.get(1)?,
        name: r.get(2)?,
        host: r.get(3)?,
        port: u16::try_from(port).unwrap_or(22),
        username: r.get(5)?,
        auth_type: AuthType::parse(&auth),
        key_source: key_source.as_deref().map(KeySource::parse),
        key_path: r.get(8)?,
        sort_order: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
        last_connected_at: r.get(12)?,
        ssh_options: r.get(13)?,
        has_password: r.get(14)?,
        has_passphrase: r.get(15)?,
        has_private_key: r.get(16)?,
    })
}

pub fn list(conn: &Connection) -> AppResult<Vec<ConnectionRecord>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT} ORDER BY c.sort_order, c.name COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map([], map_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<ConnectionRecord> {
    conn.query_row(&format!("{SELECT} WHERE c.id = ?1"), [id], map_row)
        .optional()?
        .ok_or_else(|| AppError::not_found("Connection"))
}

pub fn insert(conn: &Connection, f: &ValidFields) -> AppResult<String> {
    let id = new_id();
    let ts = now();
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM connections",
        [],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO connections
            (id, group_id, name, host, port, username, auth_type, key_source, key_path,
             sort_order, created_at, updated_at, ssh_options)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, ?12)",
        params![
            id,
            f.group_id,
            f.name,
            f.host,
            f.port,
            f.username,
            f.auth_type.as_str(),
            f.key_source.map(KeySource::as_str),
            f.key_path,
            next_order,
            ts,
            f.ssh_options
        ],
    )?;
    Ok(id)
}

pub fn update(conn: &Connection, id: &str, f: &ValidFields) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE connections SET group_id = ?2, name = ?3, host = ?4, port = ?5, username = ?6,
             auth_type = ?7, key_source = ?8, key_path = ?9, updated_at = ?10, ssh_options = ?11
         WHERE id = ?1",
        params![
            id,
            f.group_id,
            f.name,
            f.host,
            f.port,
            f.username,
            f.auth_type.as_str(),
            f.key_source.map(KeySource::as_str),
            f.key_path,
            now(),
            f.ssh_options
        ],
    )?;
    if n == 0 {
        return Err(AppError::not_found("Connection"));
    }
    Ok(())
}

pub fn set_group(conn: &Connection, id: &str, group_id: Option<&str>) -> AppResult<()> {
    if let Some(g) = group_id
        && !groups::exists(conn, g)?
    {
        return Err(AppError::validation("Selected group does not exist"));
    }
    let n = conn.execute(
        "UPDATE connections SET group_id = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, group_id, now()],
    )?;
    if n == 0 {
        return Err(AppError::not_found("Connection"));
    }
    Ok(())
}

pub fn touch_connected(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE connections SET last_connected_at = ?2 WHERE id = ?1",
        params![id, now()],
    )?;
    Ok(())
}

/// Deletes the connection row. Secrets are removed by the caller in the same transaction.
pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    let n = conn.execute("DELETE FROM connections WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(AppError::not_found("Connection"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Db;

    fn fields() -> ConnectionFields {
        ConnectionFields {
            group_id: None,
            name: " web-01 ".into(),
            host: "192.168.1.10".into(),
            port: 22,
            username: "deploy".into(),
            auth_type: AuthType::Key,
            key_source: Some(KeySource::Path),
            key_path: Some("C:/keys/id_ed25519".into()),
            ssh_options: None,
        }
    }

    #[test]
    fn validate_rules() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn().unwrap();
        let v = validate(&conn, &fields()).unwrap();
        assert_eq!(v.name, "web-01");

        let mut f = fields();
        f.host = "bad host!".into();
        assert!(validate(&conn, &f).is_err());

        let mut f = fields();
        f.host = "::1".into();
        assert!(validate(&conn, &f).is_ok());

        let mut f = fields();
        f.key_path = None;
        assert!(validate(&conn, &f).is_err());

        let mut f = fields();
        f.group_id = Some("nope".into());
        assert!(validate(&conn, &f).is_err());

        let mut f = fields();
        f.auth_type = AuthType::Password;
        let v = validate(&conn, &f).unwrap();
        assert!(v.key_source.is_none() && v.key_path.is_none());
    }

    #[test]
    fn crud_and_group_delete_sets_null() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn().unwrap();
        let g = groups::create(&conn, "Prod").unwrap();
        let mut f = fields();
        f.group_id = Some(g.id.clone());
        let v = validate(&conn, &f).unwrap();
        let id = insert(&conn, &v).unwrap();
        let rec = get(&conn, &id).unwrap();
        assert_eq!(rec.group_id.as_deref(), Some(g.id.as_str()));
        assert!(!rec.has_password);

        groups::delete(&conn, &g.id).unwrap();
        assert!(get(&conn, &id).unwrap().group_id.is_none());

        let mut f2 = fields();
        f2.name = "renamed".into();
        update(&conn, &id, &validate(&conn, &f2).unwrap()).unwrap();
        assert_eq!(get(&conn, &id).unwrap().name, "renamed");
        assert_eq!(list(&conn).unwrap().len(), 1);
        delete(&conn, &id).unwrap();
        assert!(get(&conn, &id).is_err());
    }
}
