use std::path::PathBuf;

use dune_manager_core::models::CommandFailure;
use dune_manager_core::orchestration::{RusshRunner, RusshTarget};

pub fn remote_runner(
    host: String,
    user: String,
    key_path: String,
    port: Option<u16>,
) -> Result<RusshRunner, String> {
    let mut target = RusshTarget::new(PathBuf::from(key_path), user, host);
    if let Some(p) = port {
        target.port = p;
    }
    target.validate().map_err(|err| err.message)?;
    Ok(RusshRunner::new(target))
}

pub fn runner_for_remote_kind(
    _server_type: Option<&str>,
    host: String,
    user: String,
    key_path: Option<String>,
    port: Option<u16>,
) -> Result<RusshRunner, String> {
    let key_path = key_path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "SSH private key is required for remote Ubuntu servers.".to_string())?;
    remote_runner(host, user, key_path, port)
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
