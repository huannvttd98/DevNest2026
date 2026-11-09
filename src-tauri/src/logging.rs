//! Logging: daily-rotated file in the app log dir (+ stderr in debug builds).
//!
//! Redaction is enforced by construction: secrets are only ever held in
//! `secret::Secret`, whose `Debug` output is `Secret(***)`, and no code path logs
//! secret values, key content or terminal I/O.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

pub struct LogGuard(#[allow(dead_code)] Option<WorkerGuard>);

pub fn init(log_dir: &Path) -> LogGuard {
    let filter = EnvFilter::try_from_env("DEVNEST_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,russh=warn,tao=warn,wry=warn"));

    let (file_layer, guard) = match std::fs::create_dir_all(log_dir) {
        Ok(()) => {
            let appender = tracing_appender::rolling::Builder::new()
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .filename_prefix("devnest")
                .filename_suffix("log")
                .max_log_files(14)
                .build(log_dir);
            match appender {
                Ok(appender) => {
                    let (writer, guard) = tracing_appender::non_blocking(appender);
                    (
                        Some(fmt::layer().with_ansi(false).with_writer(writer)),
                        Some(guard),
                    )
                }
                Err(_) => (None, None),
            }
        }
        Err(_) => (None, None),
    };

    let stderr_layer = cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr));

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .try_init();

    LogGuard(guard)
}
