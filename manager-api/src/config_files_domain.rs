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
        IniEntry, IniSection, UserSettingsBackupCreateResponse, UserSettingsBackupSummary,
        UserSettingsBackupsResponse, UserSettingsCatalog, UserSettingsFile,
        UserSettingsFileSummary, UserSettingsRestoreResponse, UserSettingsUpdateResponse,
    },
    state::AppState,
};

const SETTINGS_DIR: &str = "/srv/UserSettings";
const BACKUP_DIR: &str = "/srv/UserSettings/.dune-manager-backups";
const LIVE_GAME_SETTINGS_PATH: &str = "/srv/Config/LinuxServer/Game.ini";
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
    let _ = create_user_settings_backup_for_kind(state, kind).await?;
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

pub async fn read_deep_desert_pvp_partition_ids(state: &AppState) -> Result<Vec<i64>, ApiError> {
    let content = exec_filebrowser(
        state,
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!(
                "set -eu\nfile={}\nif [ ! -f \"$file\" ]; then exit 0; fi\ncat \"$file\"\n",
                sh_single_quoted(&UserSettingsKind::Game.remote_path()),
            ),
        ],
        None,
    )
    .await?;
    Ok(parse_deep_desert_pvp_partition_ids(&content))
}

pub async fn write_deep_desert_pvp_settings(
    state: &AppState,
    pvp_partition_ids: &[i64],
) -> Result<(), ApiError> {
    if pvp_partition_ids.iter().any(|id| *id <= 0) {
        return Err(ApiError::bad_request(
            "PvP partition IDs must be positive integers",
        ));
    }
    let pvp_ids = pvp_partition_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    exec_filebrowser(
        state,
        vec![
            "sh".to_string(),
            "-c".to_string(),
            deep_desert_pvp_update_script(&pvp_ids),
        ],
        None,
    )
    .await?;
    Ok(())
}

pub async fn list_user_settings_backups(
    state: &AppState,
    file_id: &str,
) -> Result<UserSettingsBackupsResponse, ApiError> {
    let kind = UserSettingsKind::parse(file_id)?;
    let output = exec_filebrowser(
        state,
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!(
                "set -eu\ndir={}\nmkdir -p \"$dir\"\nfor f in \"$dir\"/{}.*.bak; do\n  [ -e \"$f\" ] || continue\n  base=${{f##*/}}\n  size=$(wc -c < \"$f\" | tr -d ' ')\n  modified=$(date -u -r \"$f\" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || echo '')\n  printf '%s\\t%s\\t%s\\n' \"$base\" \"$size\" \"$modified\"\ndone\n",
                sh_single_quoted(BACKUP_DIR),
                kind.file_name(),
            ),
        ],
        None,
    )
    .await?;

    let mut backups = parse_backup_list(&output)?;
    backups.sort_by(|left, right| right.id.cmp(&left.id));
    Ok(UserSettingsBackupsResponse {
        file: kind.id().to_string(),
        backups,
    })
}

pub async fn create_user_settings_backup(
    state: &AppState,
    file_id: &str,
) -> Result<UserSettingsBackupCreateResponse, ApiError> {
    let kind = UserSettingsKind::parse(file_id)?;
    Ok(UserSettingsBackupCreateResponse {
        backup: create_user_settings_backup_for_kind(state, kind).await?,
    })
}

pub async fn restore_user_settings_backup(
    state: &AppState,
    file_id: &str,
    backup_id: &str,
) -> Result<UserSettingsRestoreResponse, ApiError> {
    let kind = UserSettingsKind::parse(file_id)?;
    validate_backup_id(kind, backup_id)?;
    let _ = create_user_settings_backup_for_kind(state, kind).await?;
    exec_filebrowser(
        state,
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!(
                "set -eu\nbackup={}\ndest={}\n[ -f \"$backup\" ] || {{ echo 'backup not found' >&2; exit 1; }}\ncp \"$backup\" \"$dest\"\nchmod 0644 \"$dest\"\n",
                sh_single_quoted(&format!("{BACKUP_DIR}/{backup_id}")),
                sh_single_quoted(&kind.remote_path()),
            ),
        ],
        None,
    )
    .await?;

    Ok(UserSettingsRestoreResponse {
        file: read_user_settings_file(state, file_id).await?,
        restored_from: backup_id.to_string(),
        restart_recommended: true,
    })
}

async fn create_user_settings_backup_for_kind(
    state: &AppState,
    kind: UserSettingsKind,
) -> Result<UserSettingsBackupSummary, ApiError> {
    let output = exec_filebrowser(
        state,
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!(
                "set -eu\nsrc={}\ndir={}\n[ -f \"$src\" ] || {{ echo 'settings file not found' >&2; exit 1; }}\nmkdir -p \"$dir\"\nstamp=$(date -u +%Y%m%dT%H%M%SZ)\nbackup=\"$dir/{}.$stamp.$$.bak\"\ncp \"$src\" \"$backup\"\nchmod 0644 \"$backup\"\nsize=$(wc -c < \"$backup\" | tr -d ' ')\nmodified=$(date -u -r \"$backup\" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || echo '')\nprintf '%s\\t%s\\t%s\\n' \"${{backup##*/}}\" \"$size\" \"$modified\"\n",
                sh_single_quoted(&kind.remote_path()),
                sh_single_quoted(BACKUP_DIR),
                kind.file_name(),
            ),
        ],
        None,
    )
    .await?;
    parse_backup_list(&output)?
        .into_iter()
        .next()
        .ok_or_else(|| ApiError::bad_gateway("settings backup was not created"))
}

fn parse_backup_list(output: &str) -> Result<Vec<UserSettingsBackupSummary>, ApiError> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut parts = line.splitn(3, '\t');
            let id = parts.next().unwrap_or_default().to_string();
            let size_bytes = parts
                .next()
                .unwrap_or_default()
                .parse::<usize>()
                .map_err(|_| ApiError::bad_gateway("failed to parse settings backup size"))?;
            let modified_at = parts
                .next()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);
            Ok(UserSettingsBackupSummary {
                file_name: id.clone(),
                id,
                size_bytes,
                modified_at,
            })
        })
        .collect()
}

fn validate_backup_id(kind: UserSettingsKind, value: &str) -> Result<(), ApiError> {
    let expected_prefix = format!("{}.", kind.file_name());
    if !value.starts_with(&expected_prefix)
        || !value.ends_with(".bak")
        || value.contains('/')
        || value.contains('\\')
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
    {
        return Err(ApiError::bad_request("invalid settings backup id"));
    }
    Ok(())
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

fn deep_desert_pvp_update_script(pvp_ids: &str) -> String {
    format!(
        r#"
set -eu
pvp_ids={pvp_ids}

update_ini() {{
  file="$1"
  mkdir -p "$(dirname "$file")"
  touch "$file"
  backup="$file.manager-backup-$(date -u +%Y%m%dT%H%M%SZ)"
  cp "$file" "$backup"
  tmp=$(mktemp)
  awk -v ids="$pvp_ids" '
  BEGIN {{ section="[/Script/DuneSandbox.PvpPveSettings]"; insec=0; wrote=0 }}
  function write_block(    n, parts, i) {{
    if (!wrote) {{
      print section
      print "; Managed by Dune Dedicated Server Manager"
      print "m_bIsInitialized=True"
      print "m_bShouldForceEnablePvpOnAllPartitions=False"
      print "!m_PvpEnabledPartitions=ClearArray"
      n=split(ids, parts, " ")
      for (i=1; i<=n; i++) if (parts[i] != "") print "+m_PvpEnabledPartitions=" parts[i]
      print "!m_EffectivePvpEnabledPartitions=ClearArray"
      for (i=1; i<=n; i++) if (parts[i] != "") print "+m_EffectivePvpEnabledPartitions=(UID=" parts[i] ")"
      wrote=1
    }}
  }}
  $0 == section {{ insec=1; next }}
  /^\[/ {{
    if (insec) {{ write_block(); insec=0 }}
    print
    next
  }}
  insec {{ next }}
  {{ print }}
  END {{ if (insec || !wrote) write_block() }}
  ' "$file" > "$tmp"
  cp "$tmp" "$file"
  chmod 0644 "$file"
  rm -f "$tmp"
}}

update_ini {user_game}
update_ini {live_game}
"#,
        pvp_ids = sh_single_quoted(pvp_ids),
        user_game = sh_single_quoted(&UserSettingsKind::Game.remote_path()),
        live_game = sh_single_quoted(LIVE_GAME_SETTINGS_PATH),
    )
}

fn parse_deep_desert_pvp_partition_ids(content: &str) -> Vec<i64> {
    let mut ids = Vec::new();
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed == "[/Script/DuneSandbox.PvpPveSettings]";
            continue;
        }
        if !in_section || trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if key.trim().trim_start_matches('+') != "m_PvpEnabledPartitions" {
            continue;
        }
        if let Ok(id) = value.trim().parse::<i64>() {
            ids.push(id);
        }
    }
    ids.sort_unstable();
    ids.dedup();
    ids
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

    #[test]
    fn validates_backup_ids_by_settings_file() {
        assert!(
            validate_backup_id(UserSettingsKind::Game, "UserGame.ini.20260512T010203Z.bak").is_ok()
        );
        assert!(validate_backup_id(UserSettingsKind::Game, "../UserGame.ini.bak").is_err());
        assert!(validate_backup_id(
            UserSettingsKind::Game,
            "UserEngine.ini.20260512T010203Z.bak"
        )
        .is_err());
    }

    #[test]
    fn parses_backup_listing() {
        let backups =
            parse_backup_list("UserGame.ini.20260512T010203Z.bak\t42\t2026-05-12T01:02:03Z\n")
                .unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].size_bytes, 42);
        assert_eq!(
            backups[0].modified_at.as_deref(),
            Some("2026-05-12T01:02:03Z")
        );
    }

    #[test]
    fn parses_deep_desert_pvp_partition_ids() {
        let ids = parse_deep_desert_pvp_partition_ids(
            r#"
[/Script/DuneSandbox.PvpPveSettings]
m_bIsInitialized=True
!m_PvpEnabledPartitions=ClearArray
+m_PvpEnabledPartitions=29
+m_PvpEnabledPartitions=8
+m_EffectivePvpEnabledPartitions=(UID=29)
"#,
        );

        assert_eq!(ids, vec![8, 29]);
    }

    #[test]
    fn builds_deep_desert_pvp_update_script() {
        let script = deep_desert_pvp_update_script("8 29");

        assert!(script.contains("+m_PvpEnabledPartitions="));
        assert!(script.contains("+m_EffectivePvpEnabledPartitions=(UID="));
        assert!(script.contains("/srv/UserSettings/UserGame.ini"));
        assert!(script.contains("/srv/Config/LinuxServer/Game.ini"));
    }
}
