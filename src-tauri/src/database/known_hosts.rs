use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use super::now;
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnownHost {
    pub host: String,
    pub port: u16,
    pub key_type: String,
    pub fingerprint: String,
}

/// All stored keys for a host:port (one per key type).
pub fn for_host(conn: &Connection, host: &str, port: u16) -> AppResult<Vec<KnownHost>> {
    let mut stmt = conn.prepare(
        "SELECT host, port, key_type, fingerprint FROM known_hosts WHERE host = ?1 AND port = ?2",
    )?;
    let rows = stmt.query_map(params![host.to_lowercase(), port], |r| {
        let port: i64 = r.get(1)?;
        Ok(KnownHost {
            host: r.get(0)?,
            port: u16::try_from(port).unwrap_or(22),
            key_type: r.get(2)?,
            fingerprint: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Trust a host key. When `replace_all` is set (host key changed and the user accepted
/// the new key), every previously stored key for that host:port is removed first.
pub fn trust(conn: &mut Connection, k: &KnownHost, replace_all: bool) -> AppResult<()> {
    let tx = conn.transaction()?;
    let host = k.host.to_lowercase();
    if replace_all {
        tx.execute(
            "DELETE FROM known_hosts WHERE host = ?1 AND port = ?2",
            params![host, k.port],
        )?;
    }
    tx.execute(
        "INSERT INTO known_hosts (host, port, key_type, fingerprint, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(host, port, key_type) DO UPDATE SET
            fingerprint = excluded.fingerprint, added_at = excluded.added_at",
        params![host, k.port, k.key_type, k.fingerprint, now()],
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Db;

    #[test]
    fn trust_and_replace() {
        let db = Db::open_in_memory().unwrap();
        let mut conn = db.conn().unwrap();
        let k = KnownHost {
            host: "Example.COM".into(),
            port: 22,
            key_type: "ssh-ed25519".into(),
            fingerprint: "SHA256:aaa".into(),
        };
        trust(&mut conn, &k, false).unwrap();
        let rsa = KnownHost {
            key_type: "rsa-sha2-512".into(),
            fingerprint: "SHA256:rsa".into(),
            ..k.clone()
        };
        trust(&mut conn, &rsa, false).unwrap();
        assert_eq!(for_host(&conn, "example.com", 22).unwrap().len(), 2);

        let newer = KnownHost {
            fingerprint: "SHA256:bbb".into(),
            ..k.clone()
        };
        trust(&mut conn, &newer, true).unwrap();
        let all = for_host(&conn, "example.com", 22).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].fingerprint, "SHA256:bbb");
    }
}
