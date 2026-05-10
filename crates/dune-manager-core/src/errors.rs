//! Error construction helpers for command-style APIs.

use serde::Deserialize;

use crate::models::{CommandFailure, CommandResult};
use crate::security::redact_text;

/// Creates a simple command failure with no process output attached.
pub fn failure(message: impl Into<String>) -> CommandFailure {
    CommandFailure {
        message: message.into(),
        stdout: String::new(),
        stderr: String::new(),
        code: None,
    }
}

/// Creates a command failure from a completed child-process output.
///
/// Captured stdout and stderr are redacted before they are stored.
pub fn command_failure(message: impl Into<String>, output: std::process::Output) -> CommandFailure {
    CommandFailure {
        message: message.into(),
        stdout: redact_text(&String::from_utf8_lossy(&output.stdout))
            .trim()
            .to_string(),
        stderr: redact_text(&String::from_utf8_lossy(&output.stderr))
            .trim()
            .to_string(),
        code: output.status.code(),
    }
}

/// Parses JSON text and labels parse failures with the caller-provided context.
pub fn parse_json<T: for<'de> Deserialize<'de>>(text: &str, label: &str) -> CommandResult<T> {
    serde_json::from_str(text).map_err(|err| failure(format!("Failed to parse {label}: {err}")))
}
