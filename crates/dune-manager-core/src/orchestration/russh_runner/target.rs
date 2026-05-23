//! Connection target description for the russh-based remote runner.

use std::path::PathBuf;

use crate::{errors::failure, models::CommandResult};

/// Default SSH port used when callers do not specify one.
pub const DEFAULT_SSH_PORT: u16 = 22;
/// Default connection timeout in seconds.
pub const DEFAULT_CONNECT_TIMEOUT_SECONDS: u64 = 8;

/// Connection settings for opening a russh session to a remote host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RusshTarget {
    /// Path to the private key file.
    pub key_path: PathBuf,
    /// Remote username.
    pub user: String,
    /// Remote host or IP address.
    pub host: String,
    /// TCP port the SSH server listens on.
    pub port: u16,
    /// SSH connection timeout in seconds.
    pub connect_timeout_seconds: u64,
}

impl RusshTarget {
    /// Creates a target with default port and timeout.
    pub fn new(
        key_path: impl Into<PathBuf>,
        user: impl Into<String>,
        host: impl Into<String>,
    ) -> Self {
        Self {
            key_path: key_path.into(),
            user: user.into(),
            host: host.into(),
            port: DEFAULT_SSH_PORT,
            connect_timeout_seconds: DEFAULT_CONNECT_TIMEOUT_SECONDS,
        }
    }

    /// Returns the `user@host` destination string used in error messages.
    pub fn destination(&self) -> String {
        format!("{}@{}", self.user, self.host)
    }

    /// Validates that required files and connection fields are present.
    pub fn validate(&self) -> CommandResult<()> {
        if self.key_path.as_os_str().is_empty() {
            return Err(failure("ssh key path is required"));
        }
        if !self.key_path.is_file() {
            return Err(failure(format!(
                "ssh key was not found: {}",
                self.key_path.display()
            )));
        }
        if self.user.trim().is_empty() {
            return Err(failure("ssh user is required"));
        }
        if self.host.trim().is_empty() {
            return Err(failure("ssh host is required"));
        }
        if self.port == 0 {
            return Err(failure("ssh port must be non-zero"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destination_uses_user_and_host_only() {
        let target = RusshTarget::new("key", "dune", "10.0.0.4");
        assert_eq!(target.destination(), "dune@10.0.0.4");
        assert_eq!(target.port, DEFAULT_SSH_PORT);
    }

    #[test]
    fn validate_rejects_empty_fields() {
        let temp = std::env::temp_dir().join("russh-target-test-key");
        std::fs::write(&temp, b"x").unwrap();
        let mut target = RusshTarget::new(&temp, "dune", "10.0.0.4");
        target.validate().unwrap();

        target.user = String::new();
        assert!(target.validate().is_err());

        let _ = std::fs::remove_file(&temp);
    }
}
