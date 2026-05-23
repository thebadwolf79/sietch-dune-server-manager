//! Russh session helpers: shared Tokio runtime, connect, authenticate, exec.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use russh::client::{self, Handle};
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg};
use russh::{ChannelMsg, Disconnect};
use tokio::runtime::{Builder, Runtime};

use crate::errors::failure;
use crate::models::{CommandFailure, CommandResult};

use super::target::RusshTarget;

/// Type alias for the russh client handle used by the runner.
pub(crate) type SessionHandle = Handle<AcceptAllHandler>;

/// Returns the process-wide Tokio runtime used to drive russh I/O.
///
/// The sync [`crate::orchestration::RemoteCommandRunner`] callers `block_on`
/// against this runtime, so the surrounding thread must not already be inside
/// a Tokio runtime.
pub(crate) fn shared_runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("dune-russh")
            .enable_all()
            .build()
            .expect("failed to build russh tokio runtime")
    })
}

/// Russh client handler that accepts any server key.
///
/// Matches the legacy OpenSSH wrapper which used
/// `StrictHostKeyChecking=no` + `UserKnownHostsFile=NUL`.
pub(crate) struct AcceptAllHandler;

impl client::Handler for AcceptAllHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// Opens a TCP connection, performs the SSH handshake, and authenticates with
/// the configured private key.
pub(crate) async fn connect_and_authenticate(target: &RusshTarget) -> CommandResult<SessionHandle> {
    let key_pair = load_secret_key(&target.key_path, None).map_err(|err| {
        failure(format!(
            "Failed to load ssh key {}: {err}",
            target.key_path.display()
        ))
    })?;
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(60)),
        ..client::Config::default()
    });
    let addr = (target.host.as_str(), target.port);
    let connect = tokio::time::timeout(
        Duration::from_secs(target.connect_timeout_seconds),
        client::connect(config, addr, AcceptAllHandler),
    )
    .await
    .map_err(|_| {
        failure(format!(
            "ssh connect to {} timed out after {}s",
            target.destination(),
            target.connect_timeout_seconds
        ))
    })?
    .map_err(|err| {
        failure(format!(
            "ssh connect to {} failed: {err}",
            target.destination()
        ))
    })?;
    let mut session = connect;
    let rsa_hash = session
        .best_supported_rsa_hash()
        .await
        .map_err(|err| failure(format!("ssh negotiation failed: {err}")))?
        .flatten();
    let auth = session
        .authenticate_publickey(
            &target.user,
            PrivateKeyWithHashAlg::new(Arc::new(key_pair), rsa_hash),
        )
        .await
        .map_err(|err| failure(format!("ssh public-key auth failed: {err}")))?;
    if !auth.success() {
        return Err(failure(format!(
            "ssh public-key authentication was rejected for {}",
            target.destination()
        )));
    }
    Ok(session)
}

/// Runs a single command on the given session, optionally streaming `stdin`.
///
/// Returns the trimmed stdout on success. Non-zero exit produces a
/// [`CommandFailure`] populated with stdout, stderr, and the exit code.
pub(crate) async fn exec_capture(
    handle: &SessionHandle,
    command: &str,
    stdin_body: Option<&[u8]>,
) -> CommandResult<String> {
    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|err| failure(format!("ssh open channel failed: {err}")))?;
    channel
        .exec(true, command)
        .await
        .map_err(|err| failure(format!("ssh exec failed: {err}")))?;
    if let Some(body) = stdin_body {
        if !body.is_empty() {
            channel
                .data(body)
                .await
                .map_err(|err| failure(format!("ssh stdin write failed: {err}")))?;
        }
        channel
            .eof()
            .await
            .map_err(|err| failure(format!("ssh stdin close failed: {err}")))?;
    }

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut exit_code: Option<u32> = None;
    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { data } => stdout.extend_from_slice(&data[..]),
            ChannelMsg::ExtendedData { data, ext } => {
                if ext == 1 {
                    stderr.extend_from_slice(&data[..]);
                } else {
                    stdout.extend_from_slice(&data[..]);
                }
            }
            ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status),
            _ => {}
        }
    }
    let stdout_text = String::from_utf8_lossy(&stdout).into_owned();
    let stderr_text = String::from_utf8_lossy(&stderr).into_owned();
    let code = exit_code.unwrap_or(0);
    if code != 0 {
        return Err(CommandFailure {
            message: format!("ssh remote command exited with status {code}"),
            stdout: stdout_text.trim().to_string(),
            stderr: stderr_text.trim().to_string(),
            code: i32::try_from(code).ok(),
        });
    }
    Ok(stdout_text.trim().to_string())
}

/// Sends a polite SSH disconnect on the session.
pub(crate) async fn close(handle: &SessionHandle) {
    let _ = handle
        .disconnect(Disconnect::ByApplication, "client closing", "en")
        .await;
}
