//! Steam VDF parsing and build-id queries for the host-side server package.

use std::{fs, path::Path, process::Command};

use crate::{
    errors::{command_failure, failure},
    models::CommandResult,
    shell::suppress_console_window,
};

/// Steam app id for the Dune Awakening dedicated server package.
pub const SERVER_APP_ID: &str = "4754530";
pub(super) const LEGACY_SERVER_APP_ID: &str = "3104830";

pub(super) const SERVER_MANIFEST_PATH: &str = "steamapps/appmanifest_4754530.acf";
pub(super) const LEGACY_SERVER_MANIFEST_PATH: &str = "steamapps/appmanifest_3104830.acf";

/// Reads the installed Steam build id from the package manifest.
pub fn read_installed_server_build_id(install_dir: impl AsRef<Path>) -> Option<String> {
    let manifest = fs::read_to_string(install_dir.as_ref().join(SERVER_MANIFEST_PATH)).ok()?;
    parse_vdf_value(&manifest, "buildid")
}

pub(super) fn query_latest_server_build_id(steamcmd: &Path) -> CommandResult<String> {
    let mut command = Command::new(steamcmd);
    suppress_console_window(&mut command);
    let output = command
        .args([
            "+login",
            "anonymous",
            "+app_info_update",
            "1",
            "+app_info_print",
            SERVER_APP_ID,
            "+quit",
        ])
        .output()
        .map_err(|err| failure(format!("Failed to run SteamCMD app info query: {err}")))?;
    if !output.status.success() {
        return Err(command_failure("SteamCMD app info query failed", output));
    }
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_public_branch_build_id(&text)
        .or_else(|| parse_vdf_value(&text, "buildid"))
        .ok_or_else(|| failure("SteamCMD app info did not contain a public build id"))
}

pub(super) fn parse_public_branch_build_id(text: &str) -> Option<String> {
    let mut in_branches = false;
    let mut in_public = false;
    let mut depth = 0i32;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('"') && trimmed.contains("\"branches\"") {
            in_branches = true;
            depth = 0;
            continue;
        }
        if in_branches && trimmed.starts_with('"') && trimmed.contains("\"public\"") {
            in_public = true;
            depth = 0;
            continue;
        }
        if in_public {
            if let Some(value) = parse_vdf_line_value(trimmed, "buildid") {
                return Some(value);
            }
            depth += trimmed.matches('{').count() as i32;
            depth -= trimmed.matches('}').count() as i32;
            if depth < 0 || trimmed == "}" {
                in_public = false;
            }
        } else if in_branches && trimmed == "}" {
            in_branches = false;
        }
    }
    None
}

pub(super) fn parse_vdf_value(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find_map(|line| parse_vdf_line_value(line.trim(), key))
}

fn parse_vdf_line_value(line: &str, key: &str) -> Option<String> {
    let mut parts = line.split('"').filter(|part| !part.trim().is_empty());
    let found_key = parts.next()?.trim();
    if found_key != key {
        return None;
    }
    Some(parts.next()?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_and_public_branch_build_ids() {
        let manifest = r#"
"AppState"
{
  "appid" "4754530"
  "buildid" "23216207"
}
"#;
        assert_eq!(
            parse_vdf_value(manifest, "buildid").as_deref(),
            Some("23216207")
        );

        let app_info = r#"
"depots"
{
  "branches"
  {
    "beta" { "buildid" "1" }
    "public"
    {
      "buildid" "23299999"
    }
  }
}
"#;
        assert_eq!(
            parse_public_branch_build_id(app_info).as_deref(),
            Some("23299999")
        );
    }
}
