//! Bidirectional byte proxy between a local TCP connection and a russh
//! `direct-tcpip` channel.

use russh::ChannelMsg;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::orchestration::russh_runner::session::SessionHandle;

const COPY_BUFFER_BYTES: usize = 16 * 1024;

/// Opens a `direct-tcpip` channel to `remote_host:remote_port` over the given
/// SSH session and proxies bytes between it and `local`.
///
/// `originator` is reported to the SSH server as the source of the
/// forwarded connection. Errors are intentionally swallowed: this is a
/// best-effort per-connection proxy and any failure simply closes both ends.
pub(super) async fn proxy_one_connection(
    session: &SessionHandle,
    mut local: TcpStream,
    remote_host: String,
    remote_port: u16,
    originator: (String, u16),
) {
    let channel = match session
        .channel_open_direct_tcpip(
            remote_host,
            u32::from(remote_port),
            originator.0,
            u32::from(originator.1),
        )
        .await
    {
        Ok(channel) => channel,
        Err(_) => {
            let _ = local.shutdown().await;
            return;
        }
    };

    let mut channel = channel;
    let (mut local_read, mut local_write) = local.split();
    let mut buf = vec![0u8; COPY_BUFFER_BYTES];
    let mut local_eof = false;

    loop {
        tokio::select! {
            biased;
            read = local_read.read(&mut buf), if !local_eof => {
                match read {
                    Ok(0) => {
                        local_eof = true;
                        let _ = channel.eof().await;
                    }
                    Ok(n) => {
                        if channel.data(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        let _ = channel.eof().await;
                        break;
                    }
                }
            }
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { data }) => {
                        if local_write.write_all(&data[..]).await.is_err() {
                            break;
                        }
                    }
                    Some(ChannelMsg::ExtendedData { .. }) => {}
                    Some(ChannelMsg::Eof) => {
                        let _ = local_write.shutdown().await;
                    }
                    Some(ChannelMsg::ExitStatus { .. }) => {}
                    Some(ChannelMsg::Close) => break,
                    Some(_) => {}
                    None => break,
                }
            }
        }
    }
    let _ = local_write.shutdown().await;
}
