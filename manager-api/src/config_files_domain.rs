use anyhow::{Context, Result};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{AttachParams, ListParams},
    Api,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    errors::ApiError,
    models::{
        IniEntry, IniSection, UserSettingsCatalog, UserSettingsFile, UserSettingsFileSummary,
        UserSettingsUpdateResponse,
    },
    state::AppState,
};

const SETTINGS_DIR: &str = "/srv/UserSettings";
const MAX_SETTINGS_BYTES: usize = 512 * 1024;
const MISSING_MARKER: &str = "__DUNE_MANAGER_SETTINGS_FILE_MISSING__";

#[derive(Debug, Clone, Copy)]
enum UserSettingsKind {
    Engine,
    Game,
}

impl UserSettingsKind {
    fn parse(id: &str) -> Result<Self, ApiError> {
        match id {
            "engine" | "UserEngine.ini" => Ok(Self::Engine),
            "game" | "UserGame.ini" => Ok(Self::Game),
            _ => Err(ApiError::bad_request(
                "settings file must be engine or game",
            )),
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Engine => "engine",
            Self::Game => "game",
        }
    }

    fn file_name(self) -> &'static str {
        match self {
            Self::Engine => "UserEngine.ini",
            Self::Game => "UserGame.ini",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Engine => "Engine runtime overrides copied into the game server volume.",
            Self::Game => "Game runtime overrides copied into the game server volume.",
        }
    }

    fn remote_path(self) -> String {
        format!("{SETTINGS_DIR}/{}", self.file_name())
    }

    fn summary(self) -> UserSettingsFileSummary {
        UserSettingsFileSummary {
            id: self.id(),
            file_name: self.file_name(),
            description: self.description(),
        }
    }
}

pub fn user_settings_catalog() -> UserSettingsCatalog {
    UserSettingsCatalog {
        files: vec![
            UserSettingsKind::Engine.summary(),
            UserSettingsKind::Game.summary(),
        ],
    }
}

pub async fn read_user_settings_file(
    state: &AppState,
    file_id: &str,
) -> Result<UserSettingsFile, ApiError> {
    let kind = UserSettingsKind::parse(file_id)?;
    let content = exec_filebrowser(
        state,
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!(
                "set -eu\nfile={}\nif [ ! -f \"$file\" ]; then printf '%s\\n' {}; exit 0; fi\ncat \"$file\"\n",
                sh_single_quoted(&kind.remote_path()),
                sh_single_quoted(MISSING_MARKER),
            ),
        ],
        None,
    )
    .await?;
    if content.trim() == MISSING_MARKER {
        return Err(ApiError::not_found(format!(
            "{} was not found in the filebrowser volume",
            kind.file_name()
        )));
    }
    Ok(settings_file(kind, content))
}

pub async fn write_user_settings_file(
    state: &AppState,
    file_id: &str,
    content: String,
) -> Result<UserSettingsUpdateResponse, ApiError> {
    let kind = UserSettingsKind::parse(file_id)?;
    validate_settings_content(&content)?;
    exec_filebrowser(
        state,
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!(
                "set -eu\nmkdir -p {}\ntmp=$(mktemp {}/.{}.XXXXXX)\ncat > \"$tmp\"\nchmod 0644 \"$tmp\"\nmv \"$tmp\" {}\n",
                sh_single_quoted(SETTINGS_DIR),
                sh_single_quoted(SETTINGS_DIR),
                kind.file_name(),
                sh_single_quoted(&kind.remote_path()),
            ),
        ],
        Some(content.as_bytes()),
    )
    .await?;

    Ok(UserSettingsUpdateResponse {
        file: read_user_settings_file(state, file_id).await?,
        restart_recommended: true,
    })
}

fn validate_settings_content(content: &str) -> Result<(), ApiError> {
    if content.len() > MAX_SETTINGS_BYTES {
        return Err(ApiError::bad_request(format!(
            "settings file is too large; maximum size is {MAX_SETTINGS_BYTES} bytes"
        )));
    }
    if content.contains('\0') {
        return Err(ApiError::bad_request(
            "settings file cannot contain NUL bytes",
        ));
    }
    Ok(())
}

async fn exec_filebrowser(
    state: &AppState,
    command: Vec<String>,
    stdin: Option<&[u8]>,
) -> Result<String, ApiError> {
    let pod = filebrowser_pod(state).await?;
    let pods: Api<Pod> = Api::namespaced(state.client.clone(), &state.namespace);
    let params = AttachParams {
        stdin: stdin.is_some(),
        stdout: true,
        stderr: true,
        container: Some("filebrowser".to_string()),
        max_stdout_buf_size: Some(MAX_SETTINGS_BYTES + 4096),
        max_stderr_buf_size: Some(64 * 1024),
        ..AttachParams::default()
    };
    let mut attached = pods
        .exec(&pod, command, &params)
        .await
        .context("failed to exec in filebrowser pod")?;

    let mut stdin_writer = attached.stdin();
    let mut stdout_reader = attached.stdout();
    let mut stderr_reader = attached.stderr();
    let status = attached.take_status();

    if let (Some(mut writer), Some(input)) = (stdin_writer.take(), stdin) {
        writer
            .write_all(input)
            .await
            .context("failed to write settings content to filebrowser pod")?;
        writer
            .shutdown()
            .await
            .context("failed to close settings upload stream")?;
    }

    let mut stdout = String::new();
    if let Some(mut reader) = stdout_reader.take() {
        reader
            .read_to_string(&mut stdout)
            .await
            .context("failed to read filebrowser command output")?;
    }

    let mut stderr = String::new();
    if let Some(mut reader) = stderr_reader.take() {
        reader
            .read_to_string(&mut stderr)
            .await
            .context("failed to read filebrowser command stderr")?;
    }

    if let Some(status) = status {
        if let Some(status) = status.await {
            if status.status.as_deref() != Some("Success") {
                return Err(ApiError::bad_gateway(format!(
                    "filebrowser command failed: {}",
                    stderr.trim()
                )));
            }
        }
    }
    attached
        .join()
        .await
        .context("failed to finish filebrowser command")?;
    Ok(stdout)
}

async fn filebrowser_pod(state: &AppState) -> Result<String, ApiError> {
    let pods: Api<Pod> = Api::namespaced(state.client.clone(), &state.namespace);
    let list = pods
        .list(&ListParams::default().labels("role=igw-filebrowser"))
        .await
        .context("failed to list filebrowser pods")?;
    list.items
        .into_iter()
        .find(|pod| {
            pod.status
                .as_ref()
                .and_then(|status| status.phase.as_deref())
                == Some("Running")
        })
        .and_then(|pod| pod.metadata.name)
        .ok_or_else(|| ApiError::not_found("no running filebrowser pod was found"))
}

fn settings_file(kind: UserSettingsKind, content: String) -> UserSettingsFile {
    UserSettingsFile {
        id: kind.id(),
        file_name: kind.file_name(),
        path: kind.remote_path(),
        size_bytes: content.len(),
        sections: parse_ini_sections(&content),
        content,
    }
}

fn parse_ini_sections(content: &str) -> Vec<IniSection> {
    let mut sections = Vec::new();
    let mut current = IniSection {
        name: "Global".to_string(),
        entries: Vec::new(),
    };
    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() > 2 {
            if !current.entries.is_empty() || current.name != "Global" {
                sections.push(current);
            }
            current = IniSection {
                name: trimmed[1..trimmed.len() - 1].trim().to_string(),
                entries: Vec::new(),
            };
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            current.entries.push(IniEntry {
                key: key.trim().trim_start_matches('+').to_string(),
                value: value.trim().to_string(),
                line: index + 1,
            });
        }
    }
    if !current.entries.is_empty() || current.name != "Global" {
        sections.push(current);
    }
    sections
}

fn sh_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sections_without_losing_duplicate_keys() {
        let sections = parse_ini_sections(
            r#"; comment
[/Script/DuneSandbox.PvpPveSettings]
m_bShouldForceEnablePvpOnAllPartitions=False
+m_PvpEnabledPartitions=8
+m_PvpEnabledPartitions=29
"#,
        );

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "/Script/DuneSandbox.PvpPveSettings");
        assert_eq!(sections[0].entries.len(), 3);
        assert_eq!(sections[0].entries[1].key, "m_PvpEnabledPartitions");
        assert_eq!(sections[0].entries[1].value, "8");
    }

    #[test]
    fn rejects_oversized_settings_content() {
        let content = "x".repeat(MAX_SETTINGS_BYTES + 1);
        assert!(validate_settings_content(&content).is_err());
    }
}
