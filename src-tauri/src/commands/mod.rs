pub mod app;
pub mod connections;
pub mod ssh;

pub fn require_sftp_window(window: &tauri::WebviewWindow) -> crate::error::AppResult<()> {
    if window.label().starts_with("sftp-") {
        Ok(())
    } else {
        Err(crate::error::AppError::validation(
            "This command is only available in the SFTP window",
        ))
    }
}
