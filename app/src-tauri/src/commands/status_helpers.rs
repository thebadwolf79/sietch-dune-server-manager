use serde_json::Value;

use crate::commands::status_naming::{friendly_map_name, serverset_log_key};
use crate::dto::RemoteServerComponent;

pub fn pod_component(
    label: &str,
    log_key: &str,
    pods: &Value,
    matches: impl Fn(&str, &str) -> bool,
) -> RemoteServerComponent {
    let mut total = 0usize;
    let mut ready = 0usize;
    let mut restarts = 0u64;
    let mut reasons = Vec::new();
    let mut phases = Vec::new();
    for item in pods["items"].as_array().cloned().unwrap_or_default() {
        let name = item["metadata"]["name"].as_str().unwrap_or_default();
        let role = item["metadata"]["labels"]["role"]
            .as_str()
            .unwrap_or_default();
        if !matches(role, name) {
            continue;
        }
        total += 1;
        let phase = item["status"]["phase"].as_str().unwrap_or_default();
        if !phase.is_empty() {
            phases.push(phase.to_string());
        }
        let statuses = item["status"]["containerStatuses"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let pod_ready = !statuses.is_empty()
            && statuses
                .iter()
                .all(|status| status["ready"].as_bool().unwrap_or(false));
        if pod_ready || phase == "Succeeded" {
            ready += 1;
        }
        for status in statuses {
            restarts += status["restartCount"].as_u64().unwrap_or_default();
            if let Some(reason) = status["state"]["waiting"]["reason"].as_str() {
                reasons.push(reason.to_string());
            }
            if let Some(reason) = status["state"]["terminated"]["reason"].as_str() {
                if reason != "Completed" {
                    reasons.push(reason.to_string());
                }
            }
        }
    }

    if total == 0 {
        return component(
            label,
            log_key,
            "system",
            "Not present",
            "gray",
            "No matching runtime component was found.",
            vec![],
        );
    }
    let details = compact_details(vec![
        format!("{ready}/{total} pods ready"),
        if restarts > 0 {
            format!("{restarts} container restarts")
        } else {
            String::new()
        },
        if reasons.is_empty() {
            String::new()
        } else {
            format!("Reason: {}", reasons.join(", "))
        },
    ]);
    if ready == total && reasons.is_empty() {
        component(
            label,
            log_key,
            "system",
            "Ready",
            "green",
            "All pods are ready.",
            details,
        )
    } else if reasons.iter().any(|reason| is_bad_reason(reason))
        || phases.iter().any(|phase| phase == "Failed")
    {
        component(
            label,
            log_key,
            "system",
            "Problem",
            "red",
            "One or more pods are failing.",
            details,
        )
    } else {
        component(
            label,
            log_key,
            "system",
            "Starting",
            "amber",
            "Waiting for pods to become ready.",
            details,
        )
    }
}

pub fn server_resource_components(resources: &Value) -> Vec<RemoteServerComponent> {
    let mut items = resources["items"].as_array().cloned().unwrap_or_default();
    items.sort_by(|left, right| {
        left["metadata"]["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["metadata"]["name"].as_str().unwrap_or_default())
    });
    let mut output = Vec::new();
    for item in items {
        let kind = item["kind"].as_str().unwrap_or_default();
        let name = item["metadata"]["name"].as_str().unwrap_or_default();
        match kind {
            "ServerGroup" => output.push(server_group_component(&item)),
            "ServerGateway" => output.push(resource_phase_component("Gateway Resource", &item)),
            "ServerSet" => {
                if should_show_serverset(&item) {
                    output.push(serverset_component(name, &item));
                }
            }
            _ => {}
        }
    }
    output
}

fn server_group_component(item: &Value) -> RemoteServerComponent {
    let phase = item["status"]["phase"].as_str().unwrap_or("Unknown");
    phase_component(
        "Server Group",
        "server-group",
        "system",
        phase,
        format!("Server Group reports {phase}."),
        vec![],
    )
}

fn resource_phase_component(label: &str, item: &Value) -> RemoteServerComponent {
    let phase = item["status"]["phase"].as_str().unwrap_or("Unknown");
    phase_component(
        label,
        "gateway-resource",
        "system",
        phase,
        format!("{label} reports {phase}."),
        vec![],
    )
}

fn serverset_component(name: &str, item: &Value) -> RemoteServerComponent {
    let map = item["spec"]["map"].as_str().unwrap_or_default();
    let label = friendly_map_name(map, name);
    let phase = item["status"]["phase"].as_str().unwrap_or("Unknown");
    let target = item["status"]["targetReplicas"]
        .as_u64()
        .unwrap_or_default();
    let ready = item["status"]["readyReplicas"].as_u64().unwrap_or_default();
    let completed = item["status"]["completedReplicas"]
        .as_u64()
        .unwrap_or_default();
    let pods = item["status"]["pods"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let game_ready = pods
        .iter()
        .filter(|pod| pod["ready"].as_bool().unwrap_or(false))
        .count();
    let details = compact_details(vec![
        format!("{ready}/{target} Kubernetes-ready replicas"),
        format!("{completed}/{target} completed game replicas"),
        format!("{game_ready}/{target} game-ready servers"),
    ]);
    let summary =
        if phase == "Initializing" && ready >= target && target > 0 && game_ready < target as usize
        {
            "Game process is running, but game readiness has not completed.".to_string()
        } else {
            format!("{label} reports {phase}.")
        };
    phase_component(
        &label,
        &serverset_log_key(name, map),
        "map",
        phase,
        summary,
        details,
    )
}

fn should_show_serverset(item: &Value) -> bool {
    let phase = item["status"]["phase"].as_str().unwrap_or_default();
    let target = item["status"]["targetReplicas"]
        .as_u64()
        .unwrap_or_default();
    let map = item["spec"]["map"].as_str().unwrap_or_default();
    phase != "Stopped" || target > 0 || matches!(map, "Survival_1" | "Overmap" | "DeepDesert_1")
}

fn phase_component(
    label: &str,
    log_key: &str,
    category: &str,
    phase: &str,
    summary: String,
    details: Vec<String>,
) -> RemoteServerComponent {
    let normalized = phase.to_ascii_lowercase();
    let (state, tone) = match normalized.as_str() {
        "healthy" | "running" | "ready" | "available" => ("Ready", "green"),
        "stopped" | "suspended" => ("Stopped", "gray"),
        "initializing" | "reconciling" | "pending" | "starting" => ("Starting", "amber"),
        "failed" | "error" | "degraded" => ("Problem", "red"),
        _ => ("Unknown", "amber"),
    };
    component(label, log_key, category, state, tone, summary, details)
}

fn component(
    name: &str,
    log_key: &str,
    category: &str,
    state: &str,
    tone: &str,
    summary: impl Into<String>,
    details: Vec<String>,
) -> RemoteServerComponent {
    RemoteServerComponent {
        name: name.to_string(),
        log_key: log_key.to_string(),
        category: category.to_string(),
        state: state.to_string(),
        tone: tone.to_string(),
        summary: summary.into(),
        details,
    }
}

fn compact_details(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect()
}

fn is_bad_reason(reason: &str) -> bool {
    matches!(
        reason,
        "CrashLoopBackOff"
            | "ImagePullBackOff"
            | "ErrImagePull"
            | "CreateContainerConfigError"
            | "CreateContainerError"
            | "RunContainerError"
            | "OOMKilled"
            | "Error"
    )
}
