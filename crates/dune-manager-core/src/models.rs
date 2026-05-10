//! Shared command result model.

use serde::Serialize;

/// Error returned by library operations and serialized by the CLI.
#[derive(Debug, Serialize)]
pub struct CommandFailure {
    /// Human-readable failure summary.
    pub message: String,
    /// Redacted stdout captured from a failed child process, when available.
    pub stdout: String,
    /// Redacted stderr captured from a failed child process, when available.
    pub stderr: String,
    /// Child-process exit code, when the failure came from a process.
    pub code: Option<i32>,
}

/// Result type used by command-style operations.
pub type CommandResult<T> = Result<T, CommandFailure>;
