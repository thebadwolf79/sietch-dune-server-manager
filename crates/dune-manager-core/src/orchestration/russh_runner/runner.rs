//! Sync [`RemoteCommandRunner`] backed by russh with a cached session.

use std::sync::Arc;

use tokio::sync::Mutex as AsyncMutex;

use crate::models::CommandResult;
use crate::orchestration::RemoteCommandRunner;

use super::session::{
    close as close_session, connect_and_authenticate, exec_capture, shared_runtime, SessionHandle,
};
use super::target::RusshTarget;

/// Remote command runner that exposes a sync interface backed by a cached
/// russh session.
///
/// The runner keeps one SSH session alive per instance. The session is
/// established lazily on the first call and reconnected automatically if a
/// command fails (e.g. the server dropped the connection). Cloned runners
/// share the cached session, so commands issued through multiple clones are
/// serialized over a single SSH connection.
#[derive(Clone)]
pub struct RusshRunner {
    target: RusshTarget,
    session: Arc<AsyncMutex<Option<SessionHandle>>>,
}

impl std::fmt::Debug for RusshRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RusshRunner")
            .field("target", &self.target)
            .finish()
    }
}

impl RusshRunner {
    /// Creates a runner that will lazily open a session to the given target.
    pub fn new(target: RusshTarget) -> Self {
        Self {
            target,
            session: Arc::new(AsyncMutex::new(None)),
        }
    }

    /// Returns the connection target used by this runner.
    pub fn target(&self) -> &RusshTarget {
        &self.target
    }

    /// Closes the cached session if one exists.
    pub fn close(&self) {
        let session = self.session.clone();
        shared_runtime().block_on(async move {
            if let Some(handle) = session.lock().await.take() {
                close_session(&handle).await;
            }
        });
    }

    /// Runs a command while streaming arbitrary stdin bytes to the remote
    /// process. This is intended for binary payload uploads where embedding
    /// base64 in a shell script would create a very large command body.
    pub fn run_with_stdin(&self, command: &str, stdin_body: &[u8]) -> CommandResult<String> {
        let runner = self.clone();
        let command = command.to_string();
        let stdin_body = stdin_body.to_vec();
        shared_runtime()
            .block_on(async move { runner.exec_with_retry(&command, Some(&stdin_body)).await })
    }

    async fn exec_with_retry(
        &self,
        command: &str,
        stdin_body: Option<&[u8]>,
    ) -> CommandResult<String> {
        let mut guard = self.session.lock().await;
        if guard.is_none() {
            self.target.validate()?;
            *guard = Some(connect_and_authenticate(&self.target).await?);
        }
        let first_attempt = {
            let handle = guard.as_ref().expect("session populated above");
            exec_capture(handle, command, stdin_body).await
        };
        match first_attempt {
            Ok(text) => Ok(text),
            Err(err) if is_remote_command_error(&err) => Err(err),
            Err(_) => {
                if let Some(handle) = guard.take() {
                    close_session(&handle).await;
                }
                self.target.validate()?;
                *guard = Some(connect_and_authenticate(&self.target).await?);
                let handle = guard.as_ref().expect("session populated above");
                exec_capture(handle, command, stdin_body).await
            }
        }
    }
}

fn is_remote_command_error(err: &crate::models::CommandFailure) -> bool {
    err.code.is_some() || !err.stdout.is_empty() || !err.stderr.is_empty()
}

impl RemoteCommandRunner for RusshRunner {
    fn run(&self, command: &str) -> CommandResult<String> {
        let runner = self.clone();
        let command = command.to_string();
        shared_runtime().block_on(async move { runner.exec_with_retry(&command, None).await })
    }

    fn run_script(&self, script: &str) -> CommandResult<String> {
        let runner = self.clone();
        let script = script.to_string();
        shared_runtime().block_on(async move {
            runner
                .exec_with_retry("sh -s", Some(script.as_bytes()))
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runner_is_clone_and_debug() {
        let target = RusshTarget::new("key", "dune", "10.0.0.4");
        let runner = RusshRunner::new(target.clone());
        let _clone = runner.clone();
        assert_eq!(runner.target(), &target);
        assert!(format!("{runner:?}").contains("dune"));
    }
}
