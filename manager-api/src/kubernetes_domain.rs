use anyhow::{Context, Result};
use k8s_openapi::{
    api::core::v1::{Pod, Service},
    apimachinery::pkg::util::intstr::IntOrString,
};
use kube::{
    api::{ApiResource, DynamicObject, ListParams, Patch, PatchParams},
    Api,
};
use serde_json::{json, Value};

use crate::{
    errors::ApiError,
    models::{
        BattleGroupDetail, BattleGroupSummary, PodSummary, ServerSetSummary, ServicePortSummary,
        ServiceSummary, WorldLayout, WorldLayoutUpdateRequest, WorldLayoutUpdateResponse,
    },
    state::AppState,
    validation::{validate_kube_name, validate_namespace},
};

pub async fn list_pods(state: &AppState) -> Result<Vec<PodSummary>> {
    let pods: Api<Pod> = Api::namespaced(state.client.clone(), &state.namespace);
    let list = pods
        .list(&ListParams::default())
        .await
        .context("failed to list pods")?;

    Ok(list
        .items
        .into_iter()
        .map(|pod| {
            let status = pod.status.unwrap_or_default();
            let containers = pod
                .spec
                .as_ref()
                .map(|spec| {
                    spec.containers
                        .iter()
                        .map(|container| container.name.clone())
                        .collect()
                })
                .unwrap_or_default();
            let container_statuses = status.container_statuses.unwrap_or_default();
            PodSummary {
                name: pod.metadata.name.unwrap_or_default(),
                phase: status.phase.unwrap_or_default(),
                ready: !container_statuses.is_empty()
                    && container_statuses.iter().all(|container| container.ready),
                restarts: container_statuses
                    .iter()
                    .map(|container| container.restart_count)
                    .sum(),
                containers,
                node_name: pod.spec.and_then(|spec| spec.node_name),
                created_at: pod
                    .metadata
                    .creation_timestamp
                    .map(|time| time.0.to_rfc3339()),
            }
        })
        .collect())
}

pub async fn list_services(state: &AppState) -> Result<Vec<ServiceSummary>> {
    let services: Api<Service> = Api::namespaced(state.client.clone(), &state.namespace);
    let list = services
        .list(&ListParams::default())
        .await
        .context("failed to list services")?;

    Ok(list
        .items
        .into_iter()
        .map(|service| {
            let spec = service.spec.unwrap_or_default();
            ServiceSummary {
                name: service.metadata.name.unwrap_or_default(),
                service_type: spec.type_,
                cluster_ip: spec.cluster_ip,
                external_ips: spec.external_ips.unwrap_or_default(),
                ports: spec
                    .ports
                    .unwrap_or_default()
                    .into_iter()
                    .map(|port| ServicePortSummary {
                        name: port.name,
                        port: port.port,
                        target_port: port.target_port.map(int_or_string_to_string),
                        node_port: port.node_port,
                        protocol: port.protocol,
                    })
                    .collect(),
            }
        })
        .collect())
}

pub async fn list_battlegroups(state: &AppState) -> Result<Vec<BattleGroupSummary>> {
    let api: Api<DynamicObject> = Api::namespaced_with(
        state.client.clone(),
        &state.namespace,
        &battlegroup_resource(),
    );
    let list = api
        .list(&ListParams::default())
        .await
        .context("failed to list battlegroups")?;

    Ok(list
        .items
        .into_iter()
        .map(|item| battlegroup_summary(&state.namespace, item))
        .collect())
}

pub async fn get_battlegroup_object(state: &AppState, name: &str) -> Result<DynamicObject> {
    let api: Api<DynamicObject> = Api::namespaced_with(
        state.client.clone(),
        &state.namespace,
        &battlegroup_resource(),
    );
    api.get(name)
        .await
        .with_context(|| format!("failed to get battlegroup {name}"))
}

pub async fn patch_battlegroup_stop(
    state: &AppState,
    namespace: &str,
    name: &str,
    stop: bool,
) -> Result<(), ApiError> {
    validate_namespace(state, namespace)?;
    validate_kube_name(name)?;
    let api: Api<DynamicObject> = Api::namespaced_with(
        state.client.clone(),
        &state.namespace,
        &battlegroup_resource(),
    );
    api.patch(
        name,
        &PatchParams::default(),
        &Patch::Merge(json!({ "spec": { "stop": stop } })),
    )
    .await
    .with_context(|| format!("failed to patch battlegroup {name}"))?;
    Ok(())
}

pub async fn patch_battlegroup_title(
    state: &AppState,
    namespace: &str,
    name: &str,
    title: &str,
) -> Result<BattleGroupDetail, ApiError> {
    validate_namespace(state, namespace)?;
    validate_kube_name(name)?;
    let trimmed = title.trim();
    if trimmed.is_empty() || trimmed.len() > 80 {
        return Err(ApiError::bad_request(
            "server display name must be between 1 and 80 characters",
        ));
    }
    let api: Api<DynamicObject> = Api::namespaced_with(
        state.client.clone(),
        &state.namespace,
        &battlegroup_resource(),
    );
    let item = api
        .patch(
            name,
            &PatchParams::default(),
            &Patch::Merge(json!({ "spec": { "title": trimmed } })),
        )
        .await
        .with_context(|| format!("failed to patch battlegroup title {name}"))?;
    Ok(battlegroup_detail_from_object(&state.namespace, item))
}

pub async fn get_battlegroup_layout(
    state: &AppState,
    namespace: &str,
    name: &str,
) -> Result<WorldLayout, ApiError> {
    validate_namespace(state, namespace)?;
    validate_kube_name(name)?;
    let item = get_battlegroup_object(state, name).await?;
    Ok(world_layout_from_object(item, false, Vec::new()))
}

pub async fn patch_battlegroup_layout(
    state: &AppState,
    namespace: &str,
    name: &str,
    request: WorldLayoutUpdateRequest,
) -> Result<WorldLayoutUpdateResponse, ApiError> {
    validate_namespace(state, namespace)?;
    validate_kube_name(name)?;
    let pve = request.deep_desert_pve_instances.unwrap_or(0);
    let pvp = request.deep_desert_pvp_instances.unwrap_or(0);
    let deep_desert_total = pve + pvp;
    if let Some(count) = request.hagga_basin_instances {
        validate_instance_count(count, "Hagga Basin")?;
    }
    if deep_desert_total > 0 {
        validate_instance_count(deep_desert_total, "Deep Desert")?;
    }

    let item = get_battlegroup_object(state, name).await?;
    let data = serde_json::to_value(item.data.clone()).unwrap_or_else(|_| json!({}));
    let mut operations = Vec::new();
    if let Some(count) = request.hagga_basin_instances {
        append_partition_patch(&data, "Survival_1", count, &mut operations)?;
    }
    if deep_desert_total > 0
        || request.deep_desert_pve_instances.is_some()
        || request.deep_desert_pvp_instances.is_some()
    {
        append_partition_patch(
            &data,
            "DeepDesert_1",
            deep_desert_total.max(1),
            &mut operations,
        )?;
    }

    let mut battlegroup_patched = false;
    let mut patched = item;
    if !operations.is_empty() {
        let patch: json_patch::Patch = serde_json::from_value(Value::Array(operations))
            .context("failed to build JSON patch for battlegroup layout")?;
        let api: Api<DynamicObject> = Api::namespaced_with(
            state.client.clone(),
            &state.namespace,
            &battlegroup_resource(),
        );
        patched = api
            .patch(name, &PatchParams::default(), &Patch::<()>::Json(patch))
            .await
            .with_context(|| format!("failed to patch battlegroup layout {name}"))?;
        battlegroup_patched = true;
    }

    let mut warnings = Vec::new();
    if pvp > 0 {
        warnings.push(
            "Deep Desert PvP config requires the runtime config writer; this endpoint only updated partition counts.".to_string(),
        );
    }
    if request.social_hubs_enabled.is_some() {
        warnings.push(
            "Social Hubs are currently detected but not changed by this endpoint.".to_string(),
        );
    }
    let layout = world_layout_from_object(patched, battlegroup_patched, warnings.clone());
    Ok(WorldLayoutUpdateResponse {
        restart_required: layout.restart_required,
        layout,
        battlegroup_patched,
        pvp_config_updated: false,
        warnings,
    })
}

pub fn battlegroup_detail_from_object(
    default_namespace: &str,
    item: DynamicObject,
) -> BattleGroupDetail {
    let namespace = item
        .metadata
        .namespace
        .unwrap_or_else(|| default_namespace.to_string());
    let name = item.metadata.name.unwrap_or_default();
    let data = serde_json::to_value(item.data).unwrap_or_else(|_| json!({}));
    let server_sets = summarize_server_sets(&data);
    let server_image = server_sets
        .first()
        .map(|set| set.image.clone())
        .unwrap_or_default();
    let mut utility_images = Vec::new();

    for path in [
        &data["spec"]["utilities"]["director"]["spec"]["image"],
        &data["spec"]["utilities"]["serverGateway"]["spec"]["image"],
        &data["spec"]["utilities"]["textRouter"]["spec"]["image"],
        &data["spec"]["utilities"]["fileBrowser"]["spec"]["image"],
    ] {
        if let Some(image) = path.as_str() {
            utility_images.push(image.to_string());
        }
    }

    for template in data["spec"]["utilities"]["messageQueues"]["templates"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        if let Some(image) = template["spec"]["image"].as_str() {
            utility_images.push(image.to_string());
        }
    }

    BattleGroupDetail {
        namespace,
        name,
        title: data["spec"]["title"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        phase: data["status"]["phase"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        stop: data["spec"]["stop"].as_bool().unwrap_or(false),
        database_phase: string_at_paths(
            &data,
            &[
                &["status", "database", "phase"],
                &["status", "databasePhase"],
            ],
        ),
        server_group_phase: string_at_paths(
            &data,
            &[
                &["status", "serverGroup", "phase"],
                &["status", "serverGroupPhase"],
            ],
        ),
        gateway_phase: string_at_paths(
            &data,
            &[
                &["status", "serverGateway", "phase"],
                &["status", "utilities", "serverGateway", "phase"],
            ],
        ),
        director_phase: string_at_paths(
            &data,
            &[
                &["status", "director", "phase"],
                &["status", "utilities", "director", "phase"],
            ],
        ),
        server_image,
        utility_images: unique_strings(utility_images.into_iter()),
        server_sets,
    }
}

fn battlegroup_resource() -> ApiResource {
    ApiResource {
        group: "igw.funcom.com".to_string(),
        version: "v1".to_string(),
        api_version: "igw.funcom.com/v1".to_string(),
        kind: "BattleGroup".to_string(),
        plural: "battlegroups".to_string(),
    }
}

fn int_or_string_to_string(value: IntOrString) -> String {
    match value {
        IntOrString::Int(value) => value.to_string(),
        IntOrString::String(value) => value,
    }
}

fn battlegroup_summary(default_namespace: &str, item: DynamicObject) -> BattleGroupSummary {
    let namespace = item
        .metadata
        .namespace
        .unwrap_or_else(|| default_namespace.to_string());
    let name = item.metadata.name.unwrap_or_default();
    let data = serde_json::to_value(item.data).unwrap_or_else(|_| json!({}));
    let sets = data["spec"]["serverGroup"]["template"]["spec"]["sets"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let server_image = sets
        .first()
        .and_then(|set| set["image"].as_str())
        .unwrap_or_default()
        .to_string();

    BattleGroupSummary {
        namespace,
        name,
        title: data["spec"]["title"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        phase: data["status"]["phase"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        stop: data["spec"]["stop"].as_bool().unwrap_or(false),
        server_sets: sets.len(),
        server_image,
    }
}

fn summarize_server_sets(data: &Value) -> Vec<ServerSetSummary> {
    data["spec"]["serverGroup"]["template"]["spec"]["sets"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|set| ServerSetSummary {
            map: set["map"].as_str().unwrap_or_default().to_string(),
            replicas: set["replicas"].as_u64().unwrap_or_default(),
            memory_limit: set["resources"]["limits"]["memory"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            dedicated_scaling: set["dedicatedScaling"].as_bool().unwrap_or(false),
            image: set["image"].as_str().unwrap_or_default().to_string(),
        })
        .collect()
}

fn world_layout_from_object(
    item: DynamicObject,
    restart_required: bool,
    warnings: Vec<String>,
) -> WorldLayout {
    let data = serde_json::to_value(item.data).unwrap_or_else(|_| json!({}));
    let survival_ids = partition_ids_for_map(&data, "Survival_1");
    let deep_desert_ids = partition_ids_for_map(&data, "DeepDesert_1");
    let server_sets = summarize_server_sets(&data);
    let social_hubs_enabled = server_sets.iter().any(|set| {
        let map = set.map.to_ascii_lowercase();
        map.contains("social") || map.contains("hub")
    });

    WorldLayout {
        hagga_basin_instances: survival_ids.len().max(1),
        social_hubs_enabled,
        deep_desert_pve_instances: deep_desert_ids.len(),
        deep_desert_pvp_instances: 0,
        deep_desert_total_instances: deep_desert_ids.len(),
        deep_desert_partition_ids: deep_desert_ids,
        restart_required,
        warnings,
    }
}

fn append_partition_patch(
    data: &Value,
    map_name: &str,
    count: usize,
    operations: &mut Vec<Value>,
) -> Result<(), ApiError> {
    let world_partitions = data["spec"]["database"]["template"]["spec"]["deployment"]["spec"]
        ["worldPartitions"]
        .as_array()
        .ok_or_else(|| ApiError::bad_request("BattleGroup worldPartitions is not an array"))?;
    let map_index = world_partitions
        .iter()
        .position(|item| item["map"].as_str() == Some(map_name))
        .ok_or_else(|| ApiError::bad_request(format!("BattleGroup has no {map_name} entry")))?;
    let current = world_partitions[map_index]["partitions"]
        .as_array()
        .ok_or_else(|| ApiError::bad_request(format!("{map_name} partitions is not an array")))?;
    if current.is_empty() {
        return Err(ApiError::bad_request(format!(
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
        let dimension = if map_name == "DeepDesert_1" {
            0
        } else {
            desired
                .iter()
                .filter_map(|item| item["dimension"].as_i64())
                .max()
                .unwrap_or(-1)
                + 1
        };
        let id = next_free_partition_id(&used_ids, &desired)?;
        let mut next = desired[0].clone();
        next["id"] = json!(id);
        next["dimension"] = json!(dimension);
        next["disable"] = json!(false);
        desired.push(next);
    }
    desired.truncate(count);
    if desired != *current {
        operations.push(json!({
            "op": "replace",
            "path": format!("/spec/database/template/spec/deployment/spec/worldPartitions/{map_index}/partitions"),
            "value": desired,
        }));
    }
    Ok(())
}

fn partition_ids_for_map(data: &Value, map_name: &str) -> Vec<i64> {
    data["spec"]["database"]["template"]["spec"]["deployment"]["spec"]["worldPartitions"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|item| item["map"].as_str() == Some(map_name))
        .and_then(|item| item["partitions"].as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["id"].as_i64())
                .collect()
        })
        .unwrap_or_default()
}

fn validate_instance_count(count: usize, label: &str) -> Result<(), ApiError> {
    if count == 0 || count > 64 {
        Err(ApiError::bad_request(format!(
            "{label} instance count must be between 1 and 64"
        )))
    } else {
        Ok(())
    }
}

fn collect_partition_ids(world_partitions: &[Value]) -> Vec<i64> {
    world_partitions
        .iter()
        .flat_map(|entry| entry["partitions"].as_array().into_iter().flatten())
        .filter_map(|partition| partition["id"].as_i64())
        .collect()
}

fn next_free_partition_id(existing: &[i64], desired: &[Value]) -> Result<i64, ApiError> {
    let mut used = existing.to_vec();
    used.extend(desired.iter().filter_map(|item| item["id"].as_i64()));
    used.into_iter()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| ApiError::bad_request("No free partition ID is available"))
}

fn unique_strings(values: impl Iterator<Item = String>) -> Vec<String> {
    let mut output = Vec::new();
    for value in values {
        if !value.is_empty() && !output.contains(&value) {
            output.push(value);
        }
    }
    output
}

fn string_at_paths(data: &Value, paths: &[&[&str]]) -> String {
    for path in paths {
        let mut current = data;
        for key in *path {
            current = &current[*key];
        }
        if let Some(value) = current.as_str().filter(|value| !value.is_empty()) {
            return value.to_string();
        }
    }
    String::new()
}
