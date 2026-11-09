use rusqlite_migration::{M, Migrations};

/// Bump together with adding a migration below.
pub const LATEST_VERSION: usize = 1;

const V1: &str = r#"
CREATE TABLE connection_groups (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE connections (
    id                TEXT PRIMARY KEY NOT NULL,
    group_id          TEXT REFERENCES connection_groups(id) ON DELETE SET NULL,
    name              TEXT NOT NULL,
    host              TEXT NOT NULL,
    port              INTEGER NOT NULL DEFAULT 22 CHECK (port BETWEEN 1 AND 65535),
    username          TEXT NOT NULL,
    auth_type         TEXT NOT NULL CHECK (auth_type IN ('password', 'key')),
    key_source        TEXT CHECK (key_source IN ('path', 'vault')),
    key_path          TEXT,
    sort_order        INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    last_connected_at TEXT
);
CREATE INDEX idx_connections_group ON connections(group_id);

-- Encrypted secrets (see vault module). owner_id = connection id.
CREATE TABLE secrets (
    owner_id    TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('password', 'passphrase', 'private_key')),
    nonce       BLOB NOT NULL,
    ciphertext  BLOB NOT NULL,
    created_at  TEXT NOT NULL,
    PRIMARY KEY (owner_id, kind)
);

CREATE TABLE known_hosts (
    host        TEXT NOT NULL,
    port        INTEGER NOT NULL,
    key_type    TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    added_at    TEXT NOT NULL,
    PRIMARY KEY (host, port, key_type)
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
"#;

pub fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(V1)])
}
