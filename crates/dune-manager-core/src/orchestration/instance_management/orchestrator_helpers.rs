//! World-partition patch builders and PvP config script generation.

use serde_json::{json, Value};

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::instance_management::{
        instance_map::InstanceMap,
        shell::{descend, sh_single_quoted},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorldPartitionUpdate {
    pub(super) partition_ids: Vec<i64>,
    pub(super) patch_required: bool,
    pub(super) patch_operations: Vec<Value>,
}

pub(super) fn build_world_partition_update(
    battlegroup: &Value,
    map: InstanceMap,
    count: usize,
) -> CommandResult<WorldPartitionUpdate> {
    let world_partitions_path = [
        "spec",
        "database",
        "template",
        "spec",
        "deployment",
        "spec",
        "worldPartitions",
    ];
    let world_partitions = descend(battlegroup, &world_partitions_path)?
        .as_array()
        .ok_or_else(|| failure("BattleGroup worldPartitions is not an array"))?;
    let map_name = map.map_name();
    let map_index = world_partitions
        .iter()
        .position(|item| item["map"].as_str() == Some(map_name))
        .ok_or_else(|| {
            failure(format!(
                "BattleGroup has no worldPartitions entry for {map_name}"
            ))
        })?;
    let entry = &world_partitions[map_index];
    let current = entry["partitions"]
        .as_array()
        .ok_or_else(|| failure(format!("{map_name} partitions is not an array")))?;
    if current.is_empty() {
        return Err(failure(format!(
            "{map_name} has no template partition to clone"
        )));
    }

    let mut desired = current.clone();
    desired.sort_by_key(|item| {
        (
            item["dimension"].as_i64().unwrap_or(i64::MAX),
            item["id"].as_i64().unwrap_or(i64::MAX),
        )
    });

    let used_ids = collect_partition_ids(world_partitions);
    while desired.len() < count {
        let dimension = next_partition_dimension(map, &desired);
        let id = next_free_partition_id(&used_ids, &desired)?;
        let mut next = desired[0].clone();
        next["id"] = json!(id);
        next["dimension"] = json!(dimension);
        next["disable"] = json!(false);
        desired.push(next);
    }
    desired.truncate(count);

    let partition_ids = desired
        .iter()
        .map(|item| {
            item["id"]
                .as_i64()
                .ok_or_else(|| failure("Desired partition is missing id"))
        })
        .collect::<CommandResult<Vec<_>>>()?;

    let patch_required = desired != *current;
    let patch_operations = if patch_required {
        vec![json!({
            "op": "replace",
            "path": format!(
                "/spec/database/template/spec/deployment/spec/worldPartitions/{map_index}/partitions"
            ),
            "value": desired,
        })]
    } else {
        Vec::new()
    };
    let mut patch_operations = patch_operations;

    if map == InstanceMap::Survival1 {
        append_server_group_set_patch(battlegroup, map, &partition_ids, &mut patch_operations)?;
    }

    Ok(WorldPartitionUpdate {
        partition_ids,
        patch_required: !patch_operations.is_empty(),
        patch_operations,
    })
}

fn append_server_group_set_patch(
    battlegroup: &Value,
    map: InstanceMap,
    partition_ids: &[i64],
    patch_operations: &mut Vec<Value>,
) -> CommandResult<()> {
    let sets_path = ["spec", "serverGroup", "template", "spec", "sets"];
    let sets = descend(battlegroup, &sets_path)?
        .as_array()
        .ok_or_else(|| failure("BattleGroup serverGroup sets is not an array"))?;
    let map_name = map.map_name();
    let set_index = sets
        .iter()
        .position(|item| item["map"].as_str() == Some(map_name))
        .ok_or_else(|| {
            failure(format!(
                "BattleGroup has no serverGroup set entry for {map_name}"
            ))
        })?;
    let set = &sets[set_index];
    let desired_replicas = partition_ids.len() as u64;
    let current_replicas = set["replicas"].as_u64();
    if current_replicas != Some(desired_replicas) {
        patch_operations.push(json!({
            "op": if set.get("replicas").is_some() { "replace" } else { "add" },
            "path": format!("/spec/serverGroup/template/spec/sets/{set_index}/replicas"),
            "value": desired_replicas,
        }));
    }

    let desired_partitions = partition_ids.iter().map(|id| json!(id)).collect::<Vec<_>>();
    let current_partitions = set.get("partitions").and_then(Value::as_array);
    if current_partitions != Some(&desired_partitions) {
        patch_operations.push(json!({
            "op": if set.get("partitions").is_some() { "replace" } else { "add" },
            "path": format!("/spec/serverGroup/template/spec/sets/{set_index}/partitions"),
            "value": desired_partitions,
        }));
    }
    Ok(())
}

fn next_partition_dimension(map: InstanceMap, desired: &[Value]) -> i64 {
    match map {
        InstanceMap::DeepDesert => 0,
        InstanceMap::Survival1 => {
            desired
                .iter()
                .filter_map(|item| item["dimension"].as_i64())
                .max()
                .unwrap_or(-1)
                + 1
        }
    }
}

pub(super) fn deep_desert_pvp_ids(partition_ids: &[i64], pvp_instance_count: usize) -> Vec<i64> {
    if pvp_instance_count == 0 {
        return Vec::new();
    }
    partition_ids
        .iter()
        .rev()
        .take(pvp_instance_count)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn collect_partition_ids(world_partitions: &[Value]) -> Vec<i64> {
    let mut ids = Vec::new();
    for entry in world_partitions {
        for partition in entry["partitions"].as_array().into_iter().flatten() {
            if let Some(id) = partition["id"].as_i64() {
                ids.push(id);
            }
        }
    }
    ids
}

fn next_free_partition_id(existing: &[i64], desired: &[Value]) -> CommandResult<i64> {
    let mut used = existing.to_vec();
    used.extend(desired.iter().filter_map(|item| item["id"].as_i64()));
    let max = used.into_iter().max().unwrap_or(0);
    max.checked_add(1)
        .ok_or_else(|| failure("No free partition ID is available"))
}

pub(super) fn write_pvp_config_script(namespace: &str, pvp_ids: &str) -> String {
    format!(
        r#"
set -eu
ns={namespace}
pvp_ids={pvp_ids}
pvc=$(sudo kubectl get pvc -n "$ns" --no-headers | awk '$1 !~ /-db-pvc$/ && $1 ~ /-pvc$/ {{ print $1; exit }}')
if [ -z "$pvc" ]; then
  echo "No shared battlegroup PVC found in $ns" >&2
  exit 1
fi
pv=$(sudo kubectl get pvc "$pvc" -n "$ns" -o jsonpath='{{.spec.volumeName}}')
pv_path=$(sudo kubectl get pv "$pv" -o jsonpath='{{.spec.local.path}}{{.spec.hostPath.path}}')
if [ -z "$pv_path" ]; then
  echo "No host path found for PVC $pvc" >&2
  exit 1
fi

update_ini() {{
  file="$1"
  sudo mkdir -p "$(dirname "$file")"
  sudo touch "$file"
  backup="$file.manager-backup-$(date +%Y%m%d%H%M%S)"
  sudo cp "$file" "$backup"
  tmp=$(mktemp)
  sudo awk -v ids="$pvp_ids" '
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
  sudo cp "$tmp" "$file"
  rm -f "$tmp"
}}

update_ini "$pv_path/Saved/UserSettings/UserGame.ini"
update_ini "$pv_path/Saved/Config/LinuxServer/Game.ini"
"#,
        namespace = sh_single_quoted(namespace),
        pvp_ids = sh_single_quoted(pvp_ids)
    )
}
