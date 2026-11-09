use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use super::{new_id, now};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

fn validate_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::validation("Group name is required"));
    }
    if name.chars().count() > 100 {
        return Err(AppError::validation(
            "Group name must be at most 100 characters",
        ));
    }
    Ok(name.to_string())
}

pub fn list(conn: &Connection) -> AppResult<Vec<Group>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, sort_order, created_at, updated_at
         FROM connection_groups ORDER BY sort_order, name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Group {
            id: r.get(0)?,
            name: r.get(1)?,
            sort_order: r.get(2)?,
            created_at: r.get(3)?,
            updated_at: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Group> {
    conn.query_row(
        "SELECT id, name, sort_order, created_at, updated_at FROM connection_groups WHERE id = ?1",
        [id],
        |r| {
            Ok(Group {
                id: r.get(0)?,
                name: r.get(1)?,
                sort_order: r.get(2)?,
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::not_found("Group"))
}

pub fn exists(conn: &Connection, id: &str) -> AppResult<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM connection_groups WHERE id = ?1",
            [id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

pub fn create(conn: &Connection, name: &str) -> AppResult<Group> {
    let name = validate_name(name)?;
    let id = new_id();
    let ts = now();
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM connection_groups",
        [],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO connection_groups (id, name, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, name, next_order, ts],
    )?;
    get(conn, &id)
}

pub fn rename(conn: &Connection, id: &str, name: &str) -> AppResult<Group> {
    let name = validate_name(name)?;
    let n = conn.execute(
        "UPDATE connection_groups SET name = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, name, now()],
    )?;
    if n == 0 {
        return Err(AppError::not_found("Group"));
    }
    get(conn, id)
}

/// Delete a group. Its connections become ungrouped (ON DELETE SET NULL).
pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    let n = conn.execute("DELETE FROM connection_groups WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(AppError::not_found("Group"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Db;

    #[test]
    fn crud() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn().unwrap();
        let a = create(&conn, "  Production ").unwrap();
        assert_eq!(a.name, "Production");
        let b = create(&conn, "Staging").unwrap();
        assert_eq!(b.sort_order, a.sort_order + 1);
        assert_eq!(list(&conn).unwrap().len(), 2);
        let a2 = rename(&conn, &a.id, "Prod").unwrap();
        assert_eq!(a2.name, "Prod");
        delete(&conn, &a.id).unwrap();
        assert_eq!(list(&conn).unwrap().len(), 1);
        assert!(create(&conn, "   ").is_err());
        assert!(delete(&conn, "missing").is_err());
    }
}
