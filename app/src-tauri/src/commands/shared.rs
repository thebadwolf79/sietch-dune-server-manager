use std::path::PathBuf;

use dune_manager_core::models::CommandFailure;
use dune_manager_core::orchestration::{RusshRunner, RusshTarget};

pub fn remote_runner(host: String, user: String, key_path: String) -> Result<RusshRunner, String> {
    let target = RusshTarget::new(PathBuf::from(key_path), user, host);
    target.validate().map_err(|err| err.message)?;
    Ok(RusshRunner::new(target))
}

pub fn runner_for_remote_kind(
    _server_type: Option<&str>,
    host: String,
    user: String,
    key_path: Option<String>,
) -> Result<RusshRunner, String> {
    let key_path = key_path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "SSH private key is required for remote Ubuntu servers.".to_string())?;
    remote_runner(host, user, key_path)
}

pub fn command_error_message(err: CommandFailure) -> String {
    let mut parts = vec![err.message];
    if !err.stderr.trim().is_empty() {
        parts.push(err.stderr);
    }
    if !err.stdout.trim().is_empty() {
        parts.push(err.stdout);
    }
    parts.join("\n")
}

pub fn sh_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
