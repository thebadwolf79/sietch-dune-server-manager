use std::env;
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};
use chrono_tz::Tz;

use crate::errors::ConfigError;

pub const BUILTIN_FALLBACK_TOKEN: &str = "Nu6VmPWUMvdPMeB7qErr";

pub const DEFAULT_BIN_DIR: &str = "/home/dune/.dune/bin";
pub const DEFAULT_DASHBOARD_HOST: &str = "127.0.0.1";
pub const DEFAULT_DASHBOARD_PORT: u16 = 29187;
pub const DEFAULT_TIME_ZONE: &str = "Europe/Amsterdam";
pub const DEFAULT_COMMAND_AUTH_TOKEN_FILE: &str = "/home/dune/.dune/state/command-auth-token";
pub const DEFAULT_DB_PATH_LINUX: &str = "/home/dune/.dune/state/dune-server-service.sqlite";

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub bin_dir: PathBuf,
    pub dashboard_host: String,
    pub dashboard_port: u16,
    pub db_path: PathBuf,
    pub time_zone: Tz,
    pub command_auth_token_file: PathBuf,
    pub namespace_override: Option<String>,
    pub mq_pod_override: Option<String>,
    pub db_pod_override: Option<String>,
    pub pg_host_override: Option<String>,
    pub pg_user_override: Option<String>,
    pub pg_db_override: Option<String>,
    pub kubectl_use_sudo: bool,
    pub steamcmd_path: Option<PathBuf>,
    pub steamcmd_download_path: Option<PathBuf>,
}

impl ServiceConfig {
    pub fn from_env() -> Result<Self> {
        load_dotenv(Path::new(".env"))?;

        let dashboard_host =
            env::var("DUNE_DASHBOARD_HOST").unwrap_or_else(|_| DEFAULT_DASHBOARD_HOST.to_string());
        let allow_external_bind = matches!(
            env::var("DUNE_ALLOW_EXTERNAL_BIND").ok().as_deref(),
            Some("1") | Some("true")
        );
        if !is_loopback_host(&dashboard_host) && !allow_external_bind {
            return Err(ConfigError::NonLoopbackBindForbidden(dashboard_host).into());
        }

        let dashboard_port = match env::var("DUNE_DASHBOARD_PORT").ok().as_deref() {
            None | Some("") => DEFAULT_DASHBOARD_PORT,
            Some(raw) => parse_port(raw)?,
        };

        let tz_raw =
            env::var("DUNE_SERVICE_TIME_ZONE").unwrap_or_else(|_| DEFAULT_TIME_ZONE.to_string());
        let time_zone = Tz::from_str(&tz_raw).map_err(|_| ConfigError::InvalidTimeZone(tz_raw))?;

        let db_path = env::var("DUNE_SERVICE_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_db_path());

        let command_auth_token_file = env::var("DUNE_COMMAND_AUTH_TOKEN_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_COMMAND_AUTH_TOKEN_FILE));

        // Default to sudo because k3s installs `kubectl` with a kubeconfig
        // (`/etc/rancher/k3s/k3s.yaml`) that is only root-readable. The
        // existing manager + tunnel commands already shell out as `sudo
        // kubectl` for the same reason. Operators can override with
        // `DUNE_KUBECTL_USE_SUDO=0`.
        let kubectl_use_sudo = match env::var("DUNE_KUBECTL_USE_SUDO").ok().as_deref() {
            Some("0") | Some("false") => false,
            _ => true,
        };

        Ok(Self {
            bin_dir: env::var("DUNE_BIN_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_BIN_DIR)),
            dashboard_host,
            dashboard_port,
            db_path,
            time_zone,
            command_auth_token_file,
            namespace_override: nonempty_env("DUNE_NAMESPACE"),
            mq_pod_override: nonempty_env("DUNE_MQ_POD"),
            db_pod_override: nonempty_env("DUNE_DB_POD"),
            pg_host_override: nonempty_env("DUNE_PG_HOST"),
            pg_user_override: nonempty_env("DUNE_PG_USER"),
            pg_db_override: nonempty_env("DUNE_PG_DB"),
            kubectl_use_sudo,
            steamcmd_path: nonempty_env("DUNE_STEAMCMD_PATH").map(PathBuf::from),
            steamcmd_download_path: nonempty_env("DUNE_STEAMCMD_DOWNLOAD_PATH").map(PathBuf::from),
        })
    }
}

/// Resolve the active command-auth token: env -> file -> builtin fallback.
/// The builtin is Funcom-confirmed harmless (see project memory).
pub fn resolve_command_auth_token(token_file: &Path) -> String {
    if let Ok(raw) = env::var("DUNE_COMMAND_AUTH_TOKEN") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Ok(contents) = fs::read_to_string(token_file) {
        let trimmed = contents.trim_end_matches(['\r', '\n']).trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    BUILTIN_FALLBACK_TOKEN.to_string()
}

fn nonempty_env(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_port(raw: &str) -> Result<u16, ConfigError> {
    raw.parse::<u16>()
        .ok()
        .filter(|p| *p >= 1)
        .ok_or_else(|| ConfigError::InvalidPort(raw.to_string()))
}

fn is_loopback_host(host: &str) -> bool {
    if host == "localhost" {
        return true;
    }
    IpAddr::from_str(host)
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

fn default_db_path() -> PathBuf {
    if cfg!(windows) {
        if let Ok(local) = env::var("LOCALAPPDATA") {
            return PathBuf::from(local)
                .join("dune-server-service")
                .join("state.sqlite");
        }
        PathBuf::from(".data").join("dune-server-service.sqlite")
    } else {
        PathBuf::from(DEFAULT_DB_PATH_LINUX)
    }
}

fn load_dotenv(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let contents =
        fs::read_to_string(path).with_context(|| format!("reading dotenv {}", path.display()))?;
    for line in contents.split('\n') {
        let Some((key, value)) = parse_dotenv_line(line) else {
            continue;
        };
        if env::var_os(&key).is_none() {
            // SAFETY: setting env vars at startup, before threads are spawned.
            unsafe { env::set_var(&key, &value) };
        }
    }
    Ok(())
}

fn parse_dotenv_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let eq = trimmed.find('=')?;
    if eq == 0 {
        return None;
    }
    let key = trimmed[..eq].trim().to_string();
    let value = trimmed[eq + 1..].trim();
    let value = unquote(value).to_string();
    Some((key, value))
}

fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_host_recognizes_common_forms() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("::1"));
        assert!(is_loopback_host("localhost"));
        assert!(!is_loopback_host("0.0.0.0"));
        assert!(!is_loopback_host("10.0.0.1"));
    }

    #[test]
    fn parse_port_validates_range() {
        assert_eq!(parse_port("29187").unwrap(), 29187);
        assert!(parse_port("0").is_err());
        assert!(parse_port("99999").is_err());
        assert!(parse_port("abc").is_err());
    }

    #[test]
    fn unquote_strips_matching_quotes() {
        assert_eq!(unquote(r#""value""#), "value");
        assert_eq!(unquote("'value'"), "value");
        assert_eq!(unquote("value"), "value");
        assert_eq!(unquote(r#""mismatch'"#), r#""mismatch'"#);
    }
}
