//! Pure-Rust SSH runner backed by the `russh` crate.
//!
//! Replaces the legacy `OpenSshRunner` that shelled out to `ssh.exe`. The
//! runner exposes the sync [`crate::orchestration::RemoteCommandRunner`]
//! interface by driving a small internal Tokio runtime, and caches one SSH
//! session per `(user, host, port, key_path)` to avoid paying TCP+auth on
//! every command.

mod runner;
pub(crate) mod session;
mod target;

pub use runner::RusshRunner;
pub use target::RusshTarget;
