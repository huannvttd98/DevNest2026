//! DevNest desktop backend.

mod commands;
mod database;
mod error;
mod logging;
mod secret;
mod services;
mod ssh;
mod state;
mod vault;

use std::sync::Arc;

use tauri::{Emitter, Manager, RunEvent};

use crate::ssh::SshManager;
use crate::ssh::manager::StatusEvent;
use crate::state::AppState;

pub const STATUS_EVENT: &str = "ssh://status";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let log_dir = app.path().app_log_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let log_guard = logging::init(&log_dir);
            app.manage(log_guard);
            tracing::info!(version = %app.package_info().version, "DevNest starting");

            let db = database::Db::open(&data_dir.join("devnest.db"), &data_dir.join("backups"))
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

            let vault = vault::Vault::open(&vault::KeyringStore);
            if let Err(e) = &vault {
                tracing::error!(error = %e, "vault unavailable");
            }

            let handle = app.handle().clone();
            let ssh = SshManager::new(Arc::new(move |ev: StatusEvent| {
                let _ = handle.emit(STATUS_EVENT, ev);
            }));

            app.manage(AppState::new(db, vault, ssh, data_dir, log_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::connections::groups_list,
            commands::connections::group_create,
            commands::connections::group_rename,
            commands::connections::group_delete,
            commands::connections::connections_list,
            commands::connections::connection_create,
            commands::connections::connection_update,
            commands::connections::connection_move,
            commands::connections::connection_delete,
            commands::ssh::ssh_test,
            commands::ssh::ssh_sessions,
            commands::ssh::ssh_disconnect,
            commands::ssh::known_host_trust,
            commands::ssh::terminal_open,
            commands::ssh::terminal_write,
            commands::ssh::terminal_resize,
            commands::ssh::terminal_close,
        ])
        .build(tauri::generate_context!())
        .expect("error while building DevNest");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            if let Some(state) = handle.try_state::<AppState>() {
                let ssh = state.ssh.clone();
                tauri::async_runtime::block_on(async move { ssh.disconnect_all().await });
            }
            tracing::info!("DevNest exited");
        }
    });
}
