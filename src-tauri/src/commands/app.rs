use serde::Serialize;
use std::path::PathBuf;
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

#[derive(Debug, Serialize)]
pub struct LocalDirectory {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<LocalFileEntry>,
}

#[derive(Debug, Serialize)]
pub struct LocalFileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[tauri::command]
pub async fn local_list_dir(
    window: tauri::WebviewWindow,
    path: String,
) -> AppResult<LocalDirectory> {
    super::require_sftp_window(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(path).canonicalize()?;
        if !path.is_dir() {
            return Err(crate::error::AppError::validation(
                "The selected local path is not a folder",
            ));
        }
        let parent = path.parent().map(|p| p.display().to_string());
        let mut entries = std::fs::read_dir(&path)?
            .map(|entry| {
                let entry = entry?;
                let file_type = entry.file_type()?;
                let metadata = entry.metadata()?;
                Ok(LocalFileEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    path: entry.path().display().to_string(),
                    is_dir: file_type.is_dir(),
                    size: if metadata.is_file() {
                        Some(metadata.len())
                    } else {
                        None
                    },
                })
            })
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(LocalDirectory {
            path: path.display().to_string(),
            parent,
            entries,
        })
    })
    .await
    .map_err(|e| crate::error::AppError::internal(format!("Cannot list local folder: {e}")))?
}
