use tauri::State;

use crate::database::connections::{self as repo, ConnectionRecord};
use crate::database::groups::{self, Group};
use crate::error::AppResult;
use crate::services::connections::{self as service, SaveConnectionInput};
use crate::state::AppState;

// ---- groups ---------------------------------------------------------------

#[tauri::command]
pub async fn groups_list(state: State<'_, AppState>) -> AppResult<Vec<Group>> {
    groups::list(&*state.db.conn()?)
}

#[tauri::command]
pub async fn group_create(state: State<'_, AppState>, name: String) -> AppResult<Group> {
    groups::create(&*state.db.conn()?, &name)
}

#[tauri::command]
pub async fn group_rename(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> AppResult<Group> {
    groups::rename(&*state.db.conn()?, &id, &name)
}

#[tauri::command]
pub async fn group_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    groups::delete(&*state.db.conn()?, &id)
}

// ---- connections ----------------------------------------------------------

#[tauri::command]
pub async fn connections_list(state: State<'_, AppState>) -> AppResult<Vec<ConnectionRecord>> {
    repo::list(&*state.db.conn()?)
}

#[tauri::command]
pub async fn connection_create(
    state: State<'_, AppState>,
    input: SaveConnectionInput,
) -> AppResult<ConnectionRecord> {
    let rec = service::save(&state.db, state.vault()?, None, &input)?;
    tracing::info!(connection_id = %rec.id, "connection created");
    Ok(rec)
}

#[tauri::command]
pub async fn connection_update(
    state: State<'_, AppState>,
    id: String,
    input: SaveConnectionInput,
) -> AppResult<ConnectionRecord> {
    let rec = service::save(&state.db, state.vault()?, Some(&id), &input)?;
    tracing::info!(connection_id = %rec.id, "connection updated");
    Ok(rec)
}

#[tauri::command]
pub async fn connection_move(
    state: State<'_, AppState>,
    id: String,
    group_id: Option<String>,
) -> AppResult<ConnectionRecord> {
    let conn = state.db.conn()?;
    repo::set_group(&conn, &id, group_id.as_deref().filter(|g| !g.is_empty()))?;
    repo::get(&conn, &id)
}

#[tauri::command]
pub async fn connection_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.ssh.disconnect(&id).await?;
    service::delete(&state.db, &id)?;
    tracing::info!(connection_id = %id, "connection deleted");
    Ok(())
}
