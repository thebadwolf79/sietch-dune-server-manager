//! Russh-backed local SSH port forwarder.
//!
//! Replaces `ssh -N -L 127.0.0.1:<local>:<remote_host>:<remote_port>` with a
//! pure-Rust local forwarder. The forwarder binds a TCP listener on
//! 127.0.0.1, opens one `direct-tcpip` channel per accepted connection over
//! the cached SSH session, and proxies bytes bidirectionally.

mod forwarder;
mod proxy;

pub use forwarder::LocalForwarder;
