//! Local SSH port forwarder backed by russh `direct-tcpip` channels.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::errors::failure;
use crate::models::CommandResult;
use crate::orchestration::russh_runner::session::{
    close as close_session, connect_and_authenticate, shared_runtime, SessionHandle,
};
use crate::orchestration::russh_runner::RusshTarget;

use super::proxy::proxy_one_connection;

/// Active local SSH port forwarder.
///
/// Dropping the forwarder requests shutdown and aborts the accept loop. To
/// shut down gracefully and wait for the loop to exit, call
/// [`LocalForwarder::stop`].
pub struct LocalForwarder {
    shutdown: watch::Sender<bool>,
    task: Option<JoinHandle<()>>,
    local_addr: SocketAddr,
    session: Arc<SessionHandle>,
}

impl LocalForwarder {
    /// Connects to `target`, binds a TCP listener on `127.0.0.1:local_port`
    /// (or a randomly chosen port if `local_port` is `0`), and spawns the
    /// background accept loop.
    pub fn start(
        target: &RusshTarget,
        local_port: u16,
        remote_host: &str,
        remote_port: u16,
    ) -> CommandResult<Self> {
        target.validate()?;
        let target = target.clone();
        let remote_host = remote_host.to_string();
        let rt = shared_runtime();
        rt.block_on(async move {
            let session = connect_and_authenticate(&target).await?;
            let listener = TcpListener::bind(("127.0.0.1", local_port))
                .await
                .map_err(|err| {
                    failure(format!(
                        "Failed to bind local tunnel port {local_port}: {err}"
                    ))
                })?;
            let local_addr = listener
                .local_addr()
                .map_err(|err| failure(format!("Failed to read local tunnel port: {err}")))?;
            let (shutdown_tx, shutdown_rx) = watch::channel(false);
            let session = Arc::new(session);
            let task = tokio::spawn(accept_loop(
                listener,
                session.clone(),
                remote_host,
                remote_port,
                shutdown_rx,
            ));
            Ok(LocalForwarder {
                shutdown: shutdown_tx,
                task: Some(task),
                local_addr,
                session,
            })
        })
    }

    /// Returns the actual bound local TCP port.
    pub fn local_port(&self) -> u16 {
        self.local_addr.port()
    }

    /// Returns `true` if the accept loop has already exited.
    pub fn is_finished(&self) -> bool {
        self.task
            .as_ref()
            .map(JoinHandle::is_finished)
            .unwrap_or(true)
    }

    /// Signals shutdown, waits for the accept loop to exit, and closes the
    /// SSH session.
    pub fn stop(mut self) {
        let _ = self.shutdown.send(true);
        if let Some(task) = self.task.take() {
            let session = self.session.clone();
            shared_runtime().block_on(async move {
                let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
                close_session(&session).await;
            });
        }
    }
}

impl Drop for LocalForwarder {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn accept_loop(
    listener: TcpListener,
    session: Arc<SessionHandle>,
    remote_host: String,
    remote_port: u16,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            biased;
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            accept = listener.accept() => {
                let (stream, peer) = match accept {
                    Ok(pair) => pair,
                    Err(_) => continue,
                };
                let session = session.clone();
                let remote_host = remote_host.clone();
                let origin_ip = peer.ip().to_string();
                let origin_port = peer.port();
                tokio::spawn(async move {
                    proxy_one_connection(
                        &session,
                        stream,
                        remote_host,
                        remote_port,
                        (origin_ip, origin_port),
                    )
                    .await;
                });
            }
        }
    }
}
