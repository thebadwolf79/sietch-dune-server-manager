use dune_manager_core::errors::failure;
use dune_manager_core::models::CommandResult;
use dune_manager_core::orchestration::{RemoteCommandRunner, RusshRunner};
use serde_json::Value;

use crate::commands::shared::sh_single_quoted;
use crate::commands::status_helpers::{pod_component, server_resource_components};
use crate::commands::status_naming::friendly_map_name;
use crate::dto::{
    RemoteBattlegroupServerStat, RemoteBattlegroupStatus, RemoteServerComponent,
    RemoteServerPackageStatus, RemoteServerStatus,
};

pub fn read_remote_server_status(
    runner: &RusshRunner,
    namespace: &str,
    battlegroup_name: &str,
) -> CommandResult<RemoteServerStatus> {
    // The vendor wrapper's `status` text output is the source of truth in
    // older operator versions, but the format keeps shifting across Funcom
    // releases (newer wrappers show the partial world name in "Status",
    // "N/M" ratios under "Director", and semantic words like "Healthy"
    // under "Uptime" — none of which match the older
    // `Running/Running/Running/Running/1h2m` shape we used to parse).
    // Read the BattleGroup CR's `status` object directly so we stay
    // pinned to the stable Kubernetes schema instead of the rotating
    // text rendering.
    let bg = runner.run_json(
        &format!(
            "sudo kubectl get battlegroup -n {} {} -o json",
            sh_single_quoted(namespace),
            sh_single_quoted(battlegroup_name),
        ),
        "remote battlegroup",
    )?;
    let battlegroup = battlegroup_status_from_json(&bg).ok_or_else(|| {
        failure(format!(
            "BattleGroup `{battlegroup_name}` returned no status object yet (likely still initialising)"
        ))
    })?;
    let package = read_guest_package_status(runner, namespace, battlegroup_name)?;
    Ok(RemoteServerStatus { battlegroup, package })
}

/// Pure function that maps a raw `kubectl get battlegroup ... -o json`
/// payload into the UI's `RemoteBattlegroupStatus`. Defensive: every field
/// has a sensible empty/false default so partially-populated status (e.g.
/// directorPhase = null while the operator is still reconciling) doesn't
/// break the page. Returns None only if there's no metadata.name at all,
/// in which case the JSON isn't a BattleGroup object.
pub(crate) fn battlegroup_status_from_json(bg: &Value) -> Option<RemoteBattlegroupStatus> {
    bg.get("metadata")?.get("name")?.as_str()?;
    let spec = bg.get("spec").cloned().unwrap_or(Value::Null);
    let status = bg.get("status").cloned().unwrap_or(Value::Null);

    let stop = spec
        .get("stop")
        .and_then(Value::as_bool)
        .or_else(|| status.get("stop").and_then(Value::as_bool))
        .unwrap_or(false);

    let server_stats = status
        .get("servers")
        .and_then(Value::as_array)
        .map(|servers| servers.iter().map(server_stat_from_json).collect())
        .unwrap_or_default();

    Some(RemoteBattlegroupStatus {
        stop,
        phase: string_field(&status, "phase"),
        database_phase: string_field(&status, "databasePhase"),
        server_group_phase: string_field(&status, "serverGroupPhase"),
        director_phase: string_field(&status, "directorPhase"),
        uptime: string_field(&status, "uptime"),
        server_stats,
    })
}

fn string_field(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

fn server_stat_from_json(server: &Value) -> RemoteBattlegroupServerStat {
    // The operator labels each entry by the map key (e.g. "Survival_1");
    // newer versions use `name`. Try both and pass through the friendly
    // alias so the UI matches what older operators printed in the wrapper
    // text table.
    let raw_map = server
        .get("map")
        .and_then(Value::as_str)
        .or_else(|| server.get("name").and_then(Value::as_str))
        .unwrap_or_default();
    let ready_str = match server.get("ready") {
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    RemoteBattlegroupServerStat {
        map: friendly_map_name(raw_map, raw_map),
        phase: string_field(server, "phase"),
        ready: ready_str,
        players: string_field(server, "players"),
        age: string_field(server, "age"),
    }
}

fn read_guest_package_status(
    runner: &RusshRunner,
    namespace: &str,
    battlegroup_name: &str,
) -> CommandResult<RemoteServerPackageStatus> {
    let script = r#"
set -u
download=/home/dune/.dune/download
manifest="$download/steamapps/appmanifest_4754530.acf"
ns=__NAMESPACE__
bg=__BATTLEGROUP__
read_vdf_value() {
  key="$1"
  file="$2"
  [ -f "$file" ] || return 0
  awk -F '"' -v wanted="$key" '$2 == wanted { print $4; exit }' "$file" 2>/dev/null || true
}
read_file() {
  file="$1"
  [ -f "$file" ] || return 0
  head -n 1 "$file" 2>/dev/null | tr -d '\r\n'
}
printf 'installedBuildId=%s\n' "$(read_vdf_value buildid "$manifest")"
printf 'battlegroupVersion=%s\n' "$(read_file "$download/images/battlegroup/version.txt")"
printf 'operatorVersion=%s\n' "$(read_file "$download/images/operators/version.txt")"
live_image=$(sudo kubectl get battlegroup "$bg" -n "$ns" -o jsonpath='{..image}' 2>/dev/null | tr ' ' '\n' | awk -F: '/self-hosting\/(igw-server|seabass-server):/ { print $NF; exit }' || true)
printf 'liveBattlegroupVersion=%s\n' "$live_image"
"#
    .replace("__NAMESPACE__", &sh_single_quoted(namespace))
    .replace("__BATTLEGROUP__", &sh_single_quoted(battlegroup_name));
    let output = runner.run_script(&script)?;
    let value = |key: &str| {
        output.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name == key && !value.trim().is_empty()).then(|| value.trim().to_string())
        })
    };
    Ok(RemoteServerPackageStatus {
        installed_build_id: value("installedBuildId"),
        battlegroup_version: value("battlegroupVersion"),
        live_battlegroup_version: value("liveBattlegroupVersion"),
        operator_version: value("operatorVersion"),
    })
}

pub fn read_remote_server_components(
    runner: &RusshRunner,
    namespace: &str,
) -> CommandResult<Vec<RemoteServerComponent>> {
    let pods = runner.run_json(
        &format!(
            "sudo kubectl get pods -n {} -o json",
            sh_single_quoted(namespace)
        ),
        "remote server pods",
    )?;
    let resources = runner.run_json(
        &format!(
            "sudo kubectl get servergroups,servergateways,serversets -n {} -o json",
            sh_single_quoted(namespace)
        ),
        "remote server resources",
    )?;

    let mut components = vec![
        pod_component("Database", "database", &pods, |role, name| {
            role.contains("database") && !name.contains("-util-")
        }),
        pod_component(
            "Database utilities",
            "database-utilities",
            &pods,
            |role, _| {
                role.contains("database-utility")
                    || role.contains("database-monitor")
                    || role.contains("database-pghero")
            },
        ),
        pod_component("Message Queue", "message-queue", &pods, |role, name| {
            role.contains("message-queue") || name.contains("-mq-")
        }),
        pod_component("Director", "director", &pods, |role, name| {
            role.contains("battlegroup-director") || name.contains("-bgd-")
        }),
        pod_component("Gateway", "gateway", &pods, |role, name| {
            role.contains("server-gateway") || name.contains("-sgw-")
        }),
        pod_component("Text Router", "text-router", &pods, |role, name| {
            role.contains("text-router") || name.contains("-tr-")
        }),
        pod_component("File Browser", "file-browser", &pods, |role, name| {
            role.contains("filebrowser") || name.contains("-fb-")
        }),
    ];
    components.extend(server_resource_components(&resources));
    Ok(components
        .into_iter()
        .filter(|component| component.state != "Not present")
        .collect())
}

pub fn remote_records_from_battlegroups(
    request: &crate::dto::RemoteConnectionRequest,
    value: &Value,
) -> Vec<crate::dto::RemoteServerRecord> {
    value
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| remote_record_from_battlegroup(request, item))
        .collect()
}

fn remote_record_from_battlegroup(
    request: &crate::dto::RemoteConnectionRequest,
    item: &Value,
) -> Option<crate::dto::RemoteServerRecord> {
    let namespace = item
        .get("metadata")?
        .get("namespace")?
        .as_str()?
        .to_string();
    let battlegroup_name = item.get("metadata")?.get("name")?.as_str()?.to_string();
    let title = item
        .get("spec")
        .and_then(|spec| spec.get("title"))
        .and_then(Value::as_str)
        .unwrap_or(&battlegroup_name)
        .to_string();
    let phase = item
        .get("status")
        .and_then(|status| status.get("phase"))
        .and_then(Value::as_str)
        .unwrap_or("Unknown")
        .to_string();
    let server_type = request
        .server_type
        .as_deref()
        .unwrap_or("ubuntu")
        .trim()
        .to_string();
    let user = request
        .user
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    Some(crate::dto::RemoteServerRecord {
        id: remote_record_id(&server_type, &request.host, request.key_path.as_deref()),
        name: title,
        host: request.host.clone(),
        user,
        key_path: request.key_path.clone().unwrap_or_default(),
        port: request.port,
        server_type,
        namespace,
        battlegroup_name: battlegroup_name.clone(),
        world_unique_name: battlegroup_name,
        phase,
    })
}

fn remote_record_id(_server_type: &str, host: &str, key_path: Option<&str>) -> String {
    format!(
        "ubuntu:{}:{}",
        host.trim().to_lowercase(),
        key_path.unwrap_or_default().trim().to_lowercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bg(spec: Value, status: Value) -> Value {
        json!({
            "metadata": {"name": "sh-test-bg", "namespace": "funcom-seabass-sh-test"},
            "spec": spec,
            "status": status,
        })
    }

    #[test]
    fn maps_reconciling_bg_with_null_director_phase() {
        // Mirrors the user-reported payload: phase Reconciling, gateway
        // Running, director not yet populated. Prior text-parse path was
        // confusing the UI into greying the Director tunnel; under direct
        // kubectl read the director_phase is just "" which the UI treats
        // as "ready enough".
        let value = bg(
            json!({"stop": false}),
            json!({
                "phase": "Reconciling",
                "serverGroupPhase": "Running",
                "directorPhase": Value::Null,
                "stop": Value::Null,
            }),
        );
        let dto = battlegroup_status_from_json(&value).expect("status maps");
        assert!(!dto.stop);
        assert_eq!(dto.phase, "Reconciling");
        assert_eq!(dto.server_group_phase, "Running");
        assert_eq!(dto.director_phase, "");
        assert_eq!(dto.uptime, "");
    }

    #[test]
    fn falls_back_to_status_stop_when_spec_missing() {
        let value = bg(json!({}), json!({"phase": "Stopped", "stop": true}));
        let dto = battlegroup_status_from_json(&value).expect("status maps");
        assert!(dto.stop);
        assert_eq!(dto.phase, "Stopped");
    }

    #[test]
    fn server_stats_pulled_from_status_servers_array() {
        let value = bg(
            json!({"stop": false}),
            json!({
                "phase": "Running",
                "servers": [
                    {"map": "Survival_1", "phase": "Running", "ready": true, "players": 3, "age": "1h"},
                    {"name": "DeepDesert_1", "phase": "Stopped", "ready": false, "players": 0},
                ]
            }),
        );
        let dto = battlegroup_status_from_json(&value).expect("status maps");
        assert_eq!(dto.server_stats.len(), 2);
        assert_eq!(dto.server_stats[0].map, friendly_map_name("Survival_1", "Survival_1"));
        assert_eq!(dto.server_stats[0].phase, "Running");
        assert_eq!(dto.server_stats[0].ready, "true");
        assert_eq!(dto.server_stats[0].players, "3");
        assert_eq!(dto.server_stats[1].map, friendly_map_name("DeepDesert_1", "DeepDesert_1"));
        assert_eq!(dto.server_stats[1].ready, "false");
        assert_eq!(dto.server_stats[1].age, "");
    }

    #[test]
    fn returns_none_when_not_a_battlegroup_resource() {
        let value = json!({"kind": "Pod", "spec": {}, "status": {}});
        assert!(battlegroup_status_from_json(&value).is_none());
    }
}
