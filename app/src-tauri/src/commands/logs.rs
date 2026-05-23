//! Frontend-facing helpers for the persisted operation log file.

use std::sync::Arc;

use tauri::State;

use crate::log_file::LogFile;

/// Appends a single row to the persisted operation log.
///
/// Frontend-originated log rows (those produced directly by React without a
/// matching Rust event) call this so the on-disk log mirrors the in-memory
/// view exactly.
#[tauri::command]
pub fn record_operation_log(
    log_file: State<'_, Arc<LogFile>>,
    level: String,
    scope: String,
    message: String,
) -> Result<(), String> {
    let allowed_levels = ["debug", "info", "warn", "error"];
    let normalized = if allowed_levels.contains(&level.as_str()) {
        level.as_str()
    } else {
        "info"
    };
    log_file
        .append(normalized, &scope, &message)
        .map_err(|err| err.to_string())
}

/// Returns the absolute path of the directory containing operation.log.
#[tauri::command]
pub fn get_logs_folder(log_file: State<'_, Arc<LogFile>>) -> String {
    log_file.dir().to_string_lossy().into_owned()
}
