use serde::Serialize;
use tauri::State;

use crate::error::AppResult;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub log_dir: String,
    /// `ready` | `created`, or an error message when the OS keyring is unavailable.
    pub vault_state: Option<crate::vault::VaultState>,
    pub vault_error: Option<String>,
}

#[tauri::command]
pub async fn app_info(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<AppInfo> {
    let (vault_state, vault_error) = match state.vault_status() {
        Ok(s) => (Some(s), None),
        Err(e) => (None, Some(e)),
    };
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        data_dir: state.data_dir.display().to_string(),
        log_dir: state.log_dir.display().to_string(),
        vault_state,
        vault_error,
    })
}
