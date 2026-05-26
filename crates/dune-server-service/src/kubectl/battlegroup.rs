use anyhow::{anyhow, Context, Result};
use serde_json::Value;

use super::{KubectlClient, ProcessResult};

/// Return the BattleGroup name in a given namespace (first match — the typical
/// install only has one per namespace).
pub async fn bg_name(kubectl: &KubectlClient, namespace: &str) -> Result<String> {
    let result = kubectl
        .run(&[
            "get",
            "battlegroups",
            "-n",
            namespace,
            "--no-headers",
            "-o",
            "custom-columns=NAME:.metadata.name",
        ])
        .await?;
    result.require_ok(&format!("kubectl get battlegroups -n {namespace}"))?;
    let name = result
        .stdout
        .split('\n')
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| anyhow!("no battlegroup found in namespace {namespace}"))?;
    Ok(name.to_string())
}

/// Read a JSON field from the BattleGroup CRD via `kubectl ... -o jsonpath`.
/// `path` should be e.g. `{.spec.stop}` or `{.spec.database.template.spec.user}`.
pub async fn bg_field(
    kubectl: &KubectlClient,
    namespace: &str,
    bg_name: &str,
    jsonpath: &str,
) -> Result<String> {
    let path_arg = format!("jsonpath={jsonpath}");
    let result = kubectl
        .run(&[
            "get",
            "battlegroup",
            bg_name,
            "-n",
            namespace,
            "-o",
            path_arg.as_str(),
        ])
        .await?;
    result.require_ok(&format!(
        "kubectl get battlegroup {bg_name} jsonpath={jsonpath}"
    ))?;
    Ok(result.stdout.trim().to_string())
}

/// Read the full BattleGroup spec as JSON.
pub async fn bg_json(kubectl: &KubectlClient, namespace: &str, bg_name: &str) -> Result<Value> {
    let result = kubectl
        .run(&["get", "battlegroup", bg_name, "-n", namespace, "-o", "json"])
        .await?;
    result.require_ok(&format!("kubectl get battlegroup {bg_name} -o json"))?;
    let value: Value = serde_json::from_str(&result.stdout)
        .with_context(|| format!("parsing battlegroup {bg_name} json"))?;
    Ok(value)
}

/// Count running pods in a namespace whose name matches a substring.
pub async fn count_pods_matching(
    kubectl: &KubectlClient,
    namespace: &str,
    substring: &str,
) -> Result<usize> {
    let result = kubectl
        .run(&[
            "get",
            "pods",
            "-n",
            namespace,
            "--no-headers",
            "-o",
            "custom-columns=NAME:.metadata.name",
        ])
        .await?;
    result.require_ok(&format!("kubectl get pods -n {namespace}"))?;
    let n = result
        .stdout
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty() && line.contains(substring))
        .count();
    Ok(n)
}

/// Get the value of a Kubernetes Secret data key (base64-decoded). Returns None
/// if the secret or key is missing.
pub async fn secret_value(
    kubectl: &KubectlClient,
    namespace: &str,
    secret_name: &str,
    key: &str,
) -> Result<Option<String>> {
    let path_arg = format!("jsonpath={{.data.{key}}}");
    let result = kubectl
        .run(&[
            "get",
            "secret",
            secret_name,
            "-n",
            namespace,
            "-o",
            path_arg.as_str(),
        ])
        .await?;
    if !result.ok() {
        return Ok(None);
    }
    let encoded = result.stdout.trim();
    if encoded.is_empty() {
        return Ok(None);
    }
    use base64::Engine as _;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .context("decoding base64 secret value")?;
    Ok(Some(String::from_utf8_lossy(&decoded).into_owned()))
}

/// Helper to interpret the structured "ok=…" output line emitted by the Erlang
/// rabbitmq publish snippet (admin/mq.rs).
pub fn extract_publish_status(result: &ProcessResult) -> bool {
    result.ok() && result.stdout.contains("publish=ok")
}
