//! Non-interactive command-line entry point and argument dispatch.

mod args;
mod dispatch;

use serde_json::json;

use crate::security::redact_json;

use self::dispatch::run_cli;

/// Runs the CLI using process arguments and returns a process exit code.
///
/// Successful commands print pretty JSON to stdout. Failures print a redacted
/// JSON error envelope to stderr.
pub fn run_cli_from_env() -> i32 {
    match run_cli(std::env::args().skip(1).collect()) {
        Ok(mut value) => {
            redact_json(&mut value);
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
            );
            0
        }
        Err(err) => {
            let mut value = json!({
                "ok": false,
                "error": err.message,
                "stdout": err.stdout,
                "stderr": err.stderr,
                "code": err.code,
            });
            redact_json(&mut value);
            eprintln!(
                "{}",
                serde_json::to_string_pretty(&value)
                    .unwrap_or_else(|_| "{\"ok\":false}".to_string())
            );
            err.code.unwrap_or(1)
        }
    }
}
