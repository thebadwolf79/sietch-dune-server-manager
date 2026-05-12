use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};

pub const DEFAULT_PORT: u16 = 8787;

#[derive(Debug, Clone)]
pub struct ManagerConfig {
    pub namespace: String,
    pub token: Option<String>,
    pub director_base_url: Option<String>,
    pub port: u16,
    pub ui_dir: PathBuf,
}

impl ManagerConfig {
    pub fn from_env() -> Result<Self> {
        let namespace = env::var("DUNE_NAMESPACE")
            .or_else(|_| env::var("POD_NAMESPACE"))
            .unwrap_or_else(|_| "default".to_string());
        let token = optional_env("MANAGER_API_TOKEN");
        let allow_no_auth = truthy_env("MANAGER_API_ALLOW_NO_AUTH");
        if token.is_none() && !allow_no_auth {
            return Err(anyhow!(
                "MANAGER_API_TOKEN is required unless MANAGER_API_ALLOW_NO_AUTH=true"
            ));
        }

        Ok(Self {
            namespace,
            token,
            director_base_url: optional_env("DIRECTOR_BASE_URL")
                .map(|url| url.trim_end_matches('/').to_string()),
            port: env::var("PORT")
                .ok()
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(DEFAULT_PORT),
            ui_dir: optional_env("MANAGER_UI_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(default_ui_dir),
        })
    }
}

fn default_ui_dir() -> PathBuf {
    if let Ok(cwd) = env::current_dir() {
        let dev_dist = cwd.join("manager-ui").join("dist");
        if dev_dist.is_dir() {
            return dev_dist;
        }
    }
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .map(|dir| dir.join("manager-ui"))
        .unwrap_or_else(|| PathBuf::from("manager-ui/dist"))
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn truthy_env(name: &str) -> bool {
    env::var(name)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}
