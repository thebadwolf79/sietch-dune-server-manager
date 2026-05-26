use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use base64::Engine as _;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};

use crate::kubectl::{ClusterCache, KubectlClient};

const EXCHANGE: &str = "heartbeats";
const ROUTING_KEY: &str = "notifications";
const USER_ID: &str = "fls";
const APP_ID: &str = "fls_backend";

static SAFE_LABEL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9_-]{0,63}$").unwrap());

#[derive(Debug, Clone, Copy)]
pub enum ShutdownType {
    Restart,
    Maintenance,
    Update,
}

impl ShutdownType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Restart => "Restart",
            Self::Maintenance => "Maintenance",
            Self::Update => "Update",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishResult {
    pub ok: bool,
    pub output: String,
}

#[derive(Clone)]
pub struct MqPublisher {
    kubectl: KubectlClient,
    cluster: ClusterCache,
    token: Arc<String>,
}

impl MqPublisher {
    pub fn new(kubectl: KubectlClient, cluster: ClusterCache, token: String) -> Self {
        Self {
            kubectl,
            cluster,
            token: Arc::new(token),
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub async fn publish_inner(&self, inner: &Value, label: &str) -> Result<PublishResult> {
        publish_inner(self, inner, label).await
    }

    pub async fn publish_service_broadcast(
        &self,
        title: &str,
        body: &str,
        duration_secs: u64,
    ) -> Result<PublishResult> {
        publish_service_broadcast(self, title, body, duration_secs).await
    }

    pub async fn publish_server_shutdown(
        &self,
        shutdown_type: ShutdownType,
        shutdown_timestamp: i64,
        frequency_secs: u64,
        duration_secs: u64,
    ) -> Result<PublishResult> {
        publish_server_shutdown(
            self,
            shutdown_type,
            shutdown_timestamp,
            frequency_secs,
            duration_secs,
        )
        .await
    }
}

/// Build the base64-encoded outer envelope expected by the seabass server-
/// command handler.
pub fn envelope_for_command(inner: &Value, token: &str) -> String {
    let inner_str = serde_json::to_string(inner).unwrap_or_else(|_| "{}".to_string());
    let outer = json!({
        "Version": 2,
        "AuthToken": token,
        "MessageContent": inner_str,
    });
    let outer_bytes = serde_json::to_vec(&outer).unwrap_or_default();
    base64::engine::general_purpose::STANDARD.encode(outer_bytes)
}

/// Build the Erlang expression that `rabbitmqctl eval` executes inside the MQ
/// pod. Byte-equivalent to the publish snippet in `mq.ts` so the server-side
/// log line stays consistent.
pub fn build_erlang_publish(payload_b64: &str, label: &str) -> String {
    let label = if SAFE_LABEL_RE.is_match(label) {
        label
    } else {
        "smgmt"
    };
    format!(
        "Outer = base64:decode(<<\"{payload_b64}\">>),\n\
         XName = rabbit_misc:r(<<\"/\">>, exchange, <<\"{EXCHANGE}\">>),\n\
         X = rabbit_exchange:lookup_or_die(XName),\n\
         MsgId = list_to_binary(\"smgmt-{label}-\" ++ integer_to_list(erlang:system_time(millisecond))),\n\
         P = {{list_to_atom(\"P_basic\"), <<\"Content\">>, undefined, [], undefined, undefined, undefined, undefined, undefined, MsgId, undefined, undefined, <<\"{USER_ID}\">>, <<\"{APP_ID}\">>, undefined}},\n\
         Content = rabbit_basic:build_content(P, Outer),\n\
         {{ok, Msg}} = rabbit_basic:message(XName, <<\"{ROUTING_KEY}\">>, Content),\n\
         Result = rabbit_queue_type:publish_at_most_once(X, Msg),\n\
         io:format(\"publish=~p exchange={EXCHANGE} routing={ROUTING_KEY} app_id={APP_ID} user_id={USER_ID} label={label}~n\", [Result]).\n",
    )
}

pub async fn publish_inner(
    publisher: &MqPublisher,
    inner: &Value,
    label: &str,
) -> Result<PublishResult> {
    let cluster = publisher.cluster.get().await?;
    let payload_b64 = envelope_for_command(inner, publisher.token());
    let erlang = build_erlang_publish(&payload_b64, label);

    let shell = "set -eu; \
        export PATH=/opt/rabbitmq/sbin:/opt/erlang/lib/erlang/bin:/bin:/usr/bin:/usr/local/bin:$PATH; \
        cat > /tmp/dune-mq-publish.erl; \
        expr=$(cat /tmp/dune-mq-publish.erl); \
        /opt/rabbitmq/sbin/rabbitmqctl eval \"$expr\"; \
        rm -f /tmp/dune-mq-publish.erl";

    let result = publisher
        .kubectl
        .run_timeout(
            &[
                "exec",
                "-i",
                "-n",
                &cluster.namespace,
                &cluster.mq_pod,
                "--",
                "sh",
                "-lc",
                shell,
            ],
            Some(&erlang),
            30,
        )
        .await
        .context("kubectl exec rabbitmqctl eval")?;

    let combined = if result.stderr.trim().is_empty() {
        result.stdout.clone()
    } else {
        format!("{}\n{}", result.stdout, result.stderr)
    };
    let scrubbed = crate::logger::redact(&combined).into_owned();
    if !result.ok() {
        return Err(anyhow!(
            "rabbitmqctl eval exited {}: {scrubbed}",
            result.exit_code
        ));
    }
    let ok = result.stdout.contains("publish=ok");
    Ok(PublishResult {
        ok,
        output: scrubbed,
    })
}

pub async fn publish_service_broadcast(
    publisher: &MqPublisher,
    title: &str,
    body: &str,
    duration_secs: u64,
) -> Result<PublishResult> {
    let entry = json!({"Title": title, "Body": body});
    let inner = json!({
        "ServerCommand": "ServiceBroadcast",
        "BroadcastType": "Generic",
        "BroadcastPayload": {
            "BroadcastDuration": duration_secs,
            "LocalizedText": [
                {"Key": "en",    "Title": title, "Body": body},
                {"Key": "en-US", "Title": title, "Body": body},
            ]
        }
    });
    let _ = entry;
    publish_inner(publisher, &inner, "service-broadcast").await
}

pub async fn publish_server_shutdown(
    publisher: &MqPublisher,
    shutdown_type: ShutdownType,
    shutdown_timestamp: i64,
    frequency_secs: u64,
    duration_secs: u64,
) -> Result<PublishResult> {
    let inner = json!({
        "ServerCommand": "ServiceBroadcast",
        "BroadcastType": "ServerShutdown",
        "BroadcastPayload": {
            "ShutdownType": shutdown_type.as_str(),
            "DateTimestamp": chrono::Utc::now().timestamp(),
            "ShutdownDuration": duration_secs,
            "ShutdownTimestamp": shutdown_timestamp,
            "BroadcastFrequency": frequency_secs,
        }
    });
    publish_inner(publisher, &inner, "server-shutdown").await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trips() {
        let inner = json!({"ServerCommand": "Foo", "X": 1});
        let b64 = envelope_for_command(&inner, "TOKEN123");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        let outer: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(outer["Version"], 2);
        assert_eq!(outer["AuthToken"], "TOKEN123");
        let inner_str = outer["MessageContent"].as_str().unwrap();
        let recovered: Value = serde_json::from_str(inner_str).unwrap();
        assert_eq!(recovered, inner);
    }

    #[test]
    fn erlang_uses_safe_label() {
        let snippet = build_erlang_publish("PAYLOAD", "test-label");
        assert!(snippet.contains("smgmt-test-label-"));
        let snippet = build_erlang_publish("PAYLOAD", "weird; rm -rf /");
        assert!(snippet.contains("smgmt-smgmt-")); // unsafe label replaced
    }

    #[test]
    fn erlang_contains_envelope_constants() {
        let s = build_erlang_publish("PAYLOAD", "x");
        assert!(s.contains("<<\"heartbeats\">>"));
        assert!(s.contains("<<\"notifications\">>"));
        assert!(s.contains("<<\"fls\">>"));
        assert!(s.contains("<<\"fls_backend\">>"));
        assert!(s.contains("<<\"Content\">>"));
    }
}
