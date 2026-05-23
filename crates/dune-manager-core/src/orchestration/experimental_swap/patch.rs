use serde_json::{json, Value};

use crate::{errors::failure, models::CommandResult};

pub(super) fn experimental_swap_patch_operations(value: &Value) -> CommandResult<Vec<Value>> {
    experimental_swap_patch_operations_for_swap(value, 30)
}

pub(super) fn experimental_swap_patch_operations_for_swap(
    value: &Value,
    swap_size_gib: u64,
) -> CommandResult<Vec<Value>> {
    let sets_path = ["spec", "serverGroup", "template", "spec", "sets"];
    let sets = value
        .pointer("/spec/serverGroup/template/spec/sets")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            failure("BattleGroup did not contain spec.serverGroup.template.spec.sets")
        })?;
    let mut operations = Vec::new();
    for (index, set) in sets.iter().enumerate() {
        let map = set["map"].as_str().unwrap_or_default();
        let profile = memory_profile_for_map(map, swap_size_gib);
        let mut base = sets_path
            .iter()
            .map(|part| (*part).to_string())
            .collect::<Vec<_>>();
        base.push(index.to_string());
        let resources = set.get("resources");
        if resources.is_none() || !resources.is_some_and(Value::is_object) {
            let mut path = base.clone();
            path.push("resources".to_string());
            operations.push(add_operation(
                &path,
                json!({
                    "limits": { "memory": profile.limit },
                    "requests": { "memory": profile.request },
                }),
            ));
            continue;
        }
        ensure_memory_value(
            set,
            &base,
            "limits",
            profile.limit.as_str(),
            &mut operations,
        );
        ensure_memory_value(
            set,
            &base,
            "requests",
            profile.request.as_str(),
            &mut operations,
        );
    }
    Ok(operations)
}

fn ensure_memory_value(
    set: &Value,
    base_path: &[String],
    resource_kind: &str,
    desired: &str,
    operations: &mut Vec<Value>,
) {
    let resource = set
        .get("resources")
        .and_then(|resources| resources.get(resource_kind));
    if resource.is_none() || !resource.is_some_and(Value::is_object) {
        let mut path = base_path.to_owned();
        path.push("resources".to_string());
        path.push(resource_kind.to_string());
        operations.push(add_operation(&path, json!({ "memory": desired })));
        return;
    }

    let current = resource
        .and_then(|value| value.get("memory"))
        .and_then(Value::as_str);
    if current == Some(desired) {
        return;
    }
    let op = if current.is_some() { "replace" } else { "add" };
    let mut path = base_path.to_owned();
    path.push("resources".to_string());
    path.push(resource_kind.to_string());
    path.push("memory".to_string());
    operations.push(json!({
        "op": op,
        "path": json_pointer(&path),
        "value": desired,
    }));
}

#[derive(Debug, Clone)]
struct MemoryProfile {
    limit: String,
    request: String,
}

fn memory_profile_for_map(map: &str, swap_size_gib: u64) -> MemoryProfile {
    match map {
        "Survival_1" => MemoryProfile {
            limit: scaled_gi_profile(20, 12, swap_size_gib),
            request: scaled_gi_profile(20, 5, swap_size_gib),
        },
        "DeepDesert_1" => MemoryProfile {
            limit: "10Gi".to_string(),
            request: scaled_gi_profile(10, 3, swap_size_gib),
        },
        _ => MemoryProfile {
            limit: "1Gi".to_string(),
            request: "200Mi".to_string(),
        },
    }
}

fn scaled_gi_profile(no_swap_gib: u64, vendor_swap_gib: u64, swap_size_gib: u64) -> String {
    const VENDOR_SWAP_GIB: u64 = 30;
    let swap = swap_size_gib.min(VENDOR_SWAP_GIB);
    let delta = no_swap_gib.saturating_sub(vendor_swap_gib);
    let reduction = (delta * swap).div_ceil(VENDOR_SWAP_GIB);
    let value = no_swap_gib.saturating_sub(reduction).max(vendor_swap_gib);
    format!("{value}Gi")
}

fn add_operation(path: &[String], value: Value) -> Value {
    json!({
        "op": "add",
        "path": json_pointer(path),
        "value": value,
    })
}

fn json_pointer(path: &[String]) -> String {
    format!(
        "/{}",
        path.iter()
            .map(|item| item.replace('~', "~0").replace('/', "~1"))
            .collect::<Vec<_>>()
            .join("/")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_sets_experimental_memory_without_jq() {
        let battlegroup = json!({
            "spec": {
                "serverGroup": {
                    "template": {
                        "spec": {
                            "sets": [
                                {
                                    "map": "Survival_1",
                                    "resources": {
                                        "limits": { "memory": "12Gi" },
                                        "requests": { "memory": "12Gi" }
                                    }
                                },
                                {
                                    "map": "DeepDesert_1",
                                    "resources": {
                                        "limits": { "memory": "15Gi" },
                                        "requests": { "memory": "15Gi" }
                                    }
                                },
                                {
                                    "map": "Overmap",
                                    "resources": {
                                        "limits": { "memory": "2Gi" }
                                    }
                                }
                            ]
                        }
                    }
                }
            }
        });

        let operations = experimental_swap_patch_operations(&battlegroup).unwrap();
        let text = serde_json::to_string(&operations).unwrap();

        assert!(text.contains("/spec/serverGroup/template/spec/sets/0/resources/requests/memory"));
        assert!(text.contains("/spec/serverGroup/template/spec/sets/1/resources/limits/memory"));
        assert!(text.contains("/spec/serverGroup/template/spec/sets/2/resources/requests"));
        assert!(text.contains("3Gi"));
        assert!(text.contains("200Mi"));
        assert!(!text.contains("jq"));
    }

    #[test]
    fn smaller_swap_uses_softer_memory_profile() {
        let battlegroup = json!({
            "spec": {
                "serverGroup": {
                    "template": {
                        "spec": {
                            "sets": [
                                {
                                    "map": "Survival_1",
                                    "resources": {
                                        "limits": { "memory": "20Gi" },
                                        "requests": { "memory": "20Gi" }
                                    }
                                },
                                {
                                    "map": "DeepDesert_1",
                                    "resources": {
                                        "limits": { "memory": "10Gi" },
                                        "requests": { "memory": "10Gi" }
                                    }
                                }
                            ]
                        }
                    }
                }
            }
        });

        let operations = experimental_swap_patch_operations_for_swap(&battlegroup, 10).unwrap();
        let text = serde_json::to_string(&operations).unwrap();

        assert!(text.contains("\"17Gi\""));
        assert!(text.contains("\"15Gi\""));
        assert!(text.contains("\"7Gi\""));
        assert!(!text.contains("\"12Gi\""));
        assert!(!text.contains("\"5Gi\""));
    }

    #[test]
    fn matching_profile_needs_no_patch() {
        let battlegroup = json!({
            "spec": {
                "serverGroup": {
                    "template": {
                        "spec": {
                            "sets": [
                                {
                                    "map": "Survival_1",
                                    "resources": {
                                        "limits": { "memory": "12Gi" },
                                        "requests": { "memory": "5Gi" }
                                    }
                                },
                                {
                                    "map": "DeepDesert_1",
                                    "resources": {
                                        "limits": { "memory": "10Gi" },
                                        "requests": { "memory": "3Gi" }
                                    }
                                }
                            ]
                        }
                    }
                }
            }
        });

        assert!(experimental_swap_patch_operations(&battlegroup)
            .unwrap()
            .is_empty());
    }
}
