use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;

use crate::kubectl::{battlegroup, ClusterCache, KubectlClient};
use crate::postgres::{search_players as pg_search_players, PgClient, Player};

/// Director (BGD) status endpoint, reachable only from inside the BGD pod.
const BGD_STATUS_URL: &str = "http://127.0.0.1:11717/v0/battlegroup";

/// Resolves the current namespace, runs the DB player search, then overlays the
/// live Director (BGD) state so players show `"grace period"` / `"transit"`
/// where the DB still reports a coarse online/offline (#14). The BGD overlay is
/// best-effort: if the endpoint is unreachable or unparseable, the DB-sourced
/// statuses are returned unchanged.
pub async fn search_players(
    pg: &Arc<PgClient>,
    kubectl: &KubectlClient,
    cluster: &ClusterCache,
    query: &str,
    limit: u32,
) -> Result<Vec<Player>> {
    let cluster = cluster.get().await?;
    let mut players = pg_search_players(pg, &cluster.namespace, query, limit).await?;

    if let Some(states) = fetch_bgd_player_states(kubectl, &cluster.namespace).await {
        for player in &mut players {
            if let Some(live) = states.status_for(&player.fls_id) {
                player.online = live.to_string();
            }
        }
    }

    Ok(players)
}

/// Live player states pulled from the BGD `/v0/battlegroup` payload, keyed by
/// FLS id. A player appears in at most one set; `status_for` applies the
/// precedence transit > grace > online.
#[derive(Debug, Default)]
struct BgdPlayerStates {
    online: HashSet<String>,
    grace: HashSet<String>,
    transit: HashSet<String>,
}

impl BgdPlayerStates {
    fn is_empty(&self) -> bool {
        self.online.is_empty() && self.grace.is_empty() && self.transit.is_empty()
    }

    /// Live status string for an FLS id, or `None` when BGD doesn't mention it
    /// (so the DB-sourced status is kept).
    fn status_for(&self, fls_id: &str) -> Option<&'static str> {
        if fls_id.is_empty() {
            return None;
        }
        if self.transit.contains(fls_id) {
            Some("transit")
        } else if self.grace.contains(fls_id) {
            Some("grace period")
        } else if self.online.contains(fls_id) {
            Some("online")
        } else {
            None
        }
    }
}

/// Fetch + parse the BGD status. Returns `None` (so callers keep DB statuses)
/// on any failure: no battlegroup, exec failure, non-JSON output, or a payload
/// with no recognizable player lists.
async fn fetch_bgd_player_states(
    kubectl: &KubectlClient,
    namespace: &str,
) -> Option<BgdPlayerStates> {
    let bg = battlegroup::bg_name(kubectl, namespace).await.ok()?;
    let deploy = format!("deployment/{bg}-bgd-deploy");
    let out = kubectl
        .run(&[
            "exec", "-n", namespace, &deploy, "--", "wget", "-qO-", BGD_STATUS_URL,
        ])
        .await
        .ok()?;
    if !out.ok() {
        tracing::debug!(
            ns = namespace,
            exit = out.exit_code,
            "BGD status exec failed; keeping DB player statuses"
        );
        return None;
    }
    let json: Value = serde_json::from_str(&out.stdout).ok()?;
    let states = parse_bgd_player_states(&json);
    if states.is_empty() {
        None
    } else {
        Some(states)
    }
}

/// Pull online / grace-period / transit FLS ids out of a BGD `/v0/battlegroup`
/// payload. The exact field names vary across BGD builds, so we scan the
/// documented shapes (`servers[].lastServerState.{players,gracePeriodPlayers,
/// transitPlayers}` plus top-level `transit`/`gracePeriod` lists) and tolerate
/// list entries that are either bare FLS-id strings or objects with an
/// id-bearing field. Unknown shapes simply yield empty sets.
fn parse_bgd_player_states(json: &Value) -> BgdPlayerStates {
    let mut states = BgdPlayerStates::default();

    for server in collect_bgd_servers(json) {
        let last = server.get("lastServerState").unwrap_or(server);
        collect_ids(last.get("players"), &mut states.online);
        for key in ["gracePeriodPlayers", "gracePeriod", "grace"] {
            collect_ids(last.get(key), &mut states.grace);
        }
        for key in ["transitPlayers", "transit", "inTransit"] {
            collect_ids(last.get(key), &mut states.transit);
        }
    }

    // Battlegroup-wide transit / grace lists (players between maps aren't on any
    // single server's roster).
    for key in ["transit", "transitPlayers", "inTransit"] {
        collect_ids(json.get(key), &mut states.transit);
    }
    for key in ["gracePeriod", "gracePeriodPlayers", "grace"] {
        collect_ids(json.get(key), &mut states.grace);
    }

    states
}

/// Flatten the per-server state objects out of a BGD `/v0/battlegroup` payload.
/// Live BGD nests servers under three map shapes rather than a flat list:
///   - `singleServerMaps`: `{ map: <server state> }`
///   - `dimensionMaps`: `{ map: { serversByDimension: { dim: <server state> } } }`
///   - `instancedMaps`: `{ map: { instances: [ <server state>, … ] } }`
/// A flat top-level `servers` array is also accepted as a fallback (older BGD
/// builds and the unit tests).
fn collect_bgd_servers(json: &Value) -> Vec<&Value> {
    let mut servers = Vec::new();

    if let Some(arr) = json
        .get("servers")
        .or_else(|| json.pointer("/battlegroup/servers"))
        .and_then(Value::as_array)
    {
        servers.extend(arr.iter());
    }
    if let Some(map) = json.get("singleServerMaps").and_then(Value::as_object) {
        servers.extend(map.values());
    }
    if let Some(map) = json.get("dimensionMaps").and_then(Value::as_object) {
        for entry in map.values() {
            if let Some(by_dim) = entry.get("serversByDimension").and_then(Value::as_object) {
                servers.extend(by_dim.values());
            }
        }
    }
    if let Some(map) = json.get("instancedMaps").and_then(Value::as_object) {
        for entry in map.values() {
            if let Some(instances) = entry.get("instances").and_then(Value::as_array) {
                servers.extend(instances.iter());
            }
        }
    }

    servers
}

/// Collect FLS ids from a JSON array whose elements are either FLS-id strings
/// or objects carrying an id under one of the common keys.
fn collect_ids(value: Option<&Value>, out: &mut HashSet<String>) {
    let Some(Value::Array(items)) = value else {
        return;
    };
    for item in items {
        if let Some(id) = player_id_from(item) {
            out.insert(id);
        }
    }
}

fn player_id_from(item: &Value) -> Option<String> {
    match item {
        Value::String(s) => {
            let t = s.trim();
            (!t.is_empty()).then(|| t.to_string())
        }
        Value::Object(_) => {
            for key in [
                "flsId",
                "fls_id",
                "playerId",
                "player_id",
                "id",
                "user",
                "funcomId",
                "funcom_id",
            ] {
                if let Some(s) = item.get(key).and_then(Value::as_str) {
                    let t = s.trim();
                    if !t.is_empty() {
                        return Some(t.to_string());
                    }
                }
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_states_from_per_server_and_top_level_lists() {
        let payload = json!({
            "servers": [
                {
                    "partitionId": 1,
                    "lastServerState": {
                        "players": ["fls-online-a", {"flsId": "fls-online-b"}],
                        "gracePeriodPlayers": ["fls-grace-a"]
                    }
                },
                {
                    "partitionId": 2,
                    "lastServerState": {
                        "players": [{"id": "fls-online-c"}]
                    }
                }
            ],
            "transit": [{"playerId": "fls-transit-a"}, "fls-transit-b"]
        });
        let states = parse_bgd_player_states(&payload);
        assert_eq!(states.status_for("fls-online-a"), Some("online"));
        assert_eq!(states.status_for("fls-online-b"), Some("online"));
        assert_eq!(states.status_for("fls-online-c"), Some("online"));
        assert_eq!(states.status_for("fls-grace-a"), Some("grace period"));
        assert_eq!(states.status_for("fls-transit-a"), Some("transit"));
        assert_eq!(states.status_for("fls-transit-b"), Some("transit"));
        assert_eq!(states.status_for("unknown"), None);
    }

    #[test]
    fn transit_and_grace_take_precedence_over_online() {
        // A player can appear on a server roster and also be flagged transit /
        // grace; the more-specific live state must win.
        let payload = json!({
            "servers": [{
                "lastServerState": {
                    "players": ["p-transit", "p-grace"],
                    "gracePeriodPlayers": ["p-grace"]
                }
            }],
            "transit": ["p-transit"]
        });
        let states = parse_bgd_player_states(&payload);
        assert_eq!(states.status_for("p-transit"), Some("transit"));
        assert_eq!(states.status_for("p-grace"), Some("grace period"));
    }

    #[test]
    fn empty_or_unknown_payload_yields_no_states() {
        assert!(parse_bgd_player_states(&json!({})).is_empty());
        assert!(parse_bgd_player_states(&json!({"servers": []})).is_empty());
        assert!(parse_bgd_player_states(&json!({"servers": [{"lastServerState": {}}]})).is_empty());
    }

    #[test]
    fn status_for_ignores_blank_id() {
        let payload = json!({"servers": [{"lastServerState": {"players": ["a"]}}]});
        let states = parse_bgd_player_states(&payload);
        assert_eq!(states.status_for(""), None);
    }

    #[test]
    fn parses_nested_live_bgd_map_shapes() {
        // The real BGD payload nests servers under singleServerMaps /
        // dimensionMaps.serversByDimension / instancedMaps.instances rather than
        // a flat `servers` array.
        let payload = json!({
            "singleServerMaps": {
                "Survival_1": {
                    "partition": {"partitionId": 1},
                    "lastServerState": {
                        "players": ["single-online"],
                        "gracePeriodPlayers": ["single-grace"]
                    }
                }
            },
            "dimensionMaps": {
                "DeepDesert": {
                    "serversByDimension": {
                        "0": {
                            "partition": {"partitionId": 10},
                            "lastServerState": {"players": ["dim-online"]}
                        }
                    }
                }
            },
            "instancedMaps": {
                "Cave": {
                    "instances": [
                        {
                            "partition": {"partitionId": 20},
                            "lastServerState": {
                                "players": ["inst-online"],
                                "transitPlayers": ["inst-transit"]
                            }
                        }
                    ]
                }
            }
        });
        let states = parse_bgd_player_states(&payload);
        assert_eq!(states.status_for("single-online"), Some("online"));
        assert_eq!(states.status_for("single-grace"), Some("grace period"));
        assert_eq!(states.status_for("dim-online"), Some("online"));
        assert_eq!(states.status_for("inst-online"), Some("online"));
        assert_eq!(states.status_for("inst-transit"), Some("transit"));
    }
}
