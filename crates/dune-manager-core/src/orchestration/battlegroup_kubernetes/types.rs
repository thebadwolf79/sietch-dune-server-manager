//! Data types for battlegroup status snapshots, pods, shells, and exported logs.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Pod/container pair discovered in a namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodContainerRef {
    /// Pod name.
    pub pod: String,
    /// Container name inside the pod.
    pub container: String,
    /// Workload role label, when present.
    pub role: String,
}

/// Candidate commands for opening a shell into a pod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodShellSpec {
    /// Kubernetes namespace.
    pub namespace: String,
    /// Pod name.
    pub pod: String,
    /// Ordered shell command candidates.
    pub commands: Vec<Vec<String>>,
}

/// Exported log file contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFile {
    /// Relative path to use when writing the log archive.
    pub relative_path: String,
    /// Log contents.
    pub contents: String,
}

/// Combined battlegroup resource and runtime status snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BattlegroupStatusSnapshot {
    /// Raw live BattleGroup custom resource JSON.
    pub battlegroup: Value,
    /// Pods and containers in the battlegroup namespace.
    pub pods: Vec<PodContainerRef>,
    /// Director NodePort, when discovered.
    pub director_node_port: Option<u16>,
}
