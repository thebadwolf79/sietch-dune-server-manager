use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls};

use crate::kubectl::{battlegroup, KubectlClient};

const DEFAULT_HOST_PORT: u16 = 15432;
const DEFAULT_CLUSTER_PORT: u16 = 5432;
const DEFAULT_DB: &str = "dune";
const DEFAULT_USER: &str = "dune";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// User-supplied overrides for the Postgres connection. Any field left as None
/// is resolved from the BattleGroup CRD / loopback probe / ClusterIP lookup.
#[derive(Debug, Clone, Default)]
pub struct PgConfig {
    pub host_override: Option<String>,
    pub user_override: Option<String>,
    pub db_override: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PgEndpoint {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct PgCredentials {
    pub user: String,
    pub password: String,
    pub database: String,
}

/// Pooled-ish Postgres handle. Holds a single connected `Client`; on connection
/// loss the next call reconnects.
pub struct PgClient {
    kubectl: KubectlClient,
    config: PgConfig,
    inner: Mutex<Option<Arc<ClientState>>>,
}

pub struct ClientState {
    client: Client,
}

impl PgClient {
    pub fn new(kubectl: KubectlClient, config: PgConfig) -> Self {
        Self {
            kubectl,
            config,
            inner: Mutex::new(None),
        }
    }

    /// Get a connected client, reconnecting if the previous connection has
    /// closed.
    pub async fn client(&self, namespace: &str) -> Result<Arc<ClientState>> {
        {
            let guard = self.inner.lock().await;
            if let Some(state) = guard.as_ref() {
                if !state.client.is_closed() {
                    return Ok(state.clone());
                }
            }
        }

        let state = self.connect(namespace).await?;
        let arc = Arc::new(state);
        let mut guard = self.inner.lock().await;
        *guard = Some(arc.clone());
        Ok(arc)
    }

    /// Create a fresh connection for operations that need exclusive session
    /// state, such as an explicit SQL transaction.
    pub async fn dedicated_client(&self, namespace: &str) -> Result<ClientState> {
        self.connect(namespace).await
    }

    async fn connect(&self, namespace: &str) -> Result<ClientState> {
        let creds = self.resolve_credentials(namespace).await?;
        let endpoint = self.resolve_endpoint(namespace).await?;

        let conninfo = format!(
            "host={} port={} user={} password={} dbname={} connect_timeout=5 application_name=dune-server-service",
            endpoint.host,
            endpoint.port,
            shell_escape_keyvalue(&creds.user),
            shell_escape_keyvalue(&creds.password),
            shell_escape_keyvalue(&creds.database),
        );

        tracing::info!(
            host = endpoint.host.as_str(),
            port = endpoint.port,
            user = creds.user.as_str(),
            db = creds.database.as_str(),
            "connecting to postgres"
        );

        let (client, connection) = tokio_postgres::connect(&conninfo, NoTls)
            .await
            .with_context(|| {
                format!(
                    "connecting to postgres at {}:{}",
                    endpoint.host, endpoint.port
                )
            })?;

        tokio::spawn(async move {
            if let Err(err) = connection.await {
                tracing::error!(error = %err, "postgres connection task ended");
            }
        });

        Ok(ClientState { client })
    }

    async fn resolve_credentials(&self, namespace: &str) -> Result<PgCredentials> {
        let bg = battlegroup::bg_name(&self.kubectl, namespace).await?;
        let user = match &self.config.user_override {
            Some(u) if !u.is_empty() => u.clone(),
            _ => {
                let raw = battlegroup::bg_field(
                    &self.kubectl,
                    namespace,
                    &bg,
                    "{.spec.database.template.spec.deployment.spec.user}",
                )
                .await
                .unwrap_or_default();
                if raw.is_empty() {
                    DEFAULT_USER.to_string()
                } else {
                    raw
                }
            }
        };
        let database = match &self.config.db_override {
            Some(d) if !d.is_empty() => d.clone(),
            _ => {
                let raw = battlegroup::bg_field(
                    &self.kubectl,
                    namespace,
                    &bg,
                    "{.spec.database.template.spec.deployment.spec.gameDatabaseName}",
                )
                .await
                .unwrap_or_default();
                if raw.is_empty() {
                    DEFAULT_DB.to_string()
                } else {
                    raw
                }
            }
        };
        let password = battlegroup::bg_field(
            &self.kubectl,
            namespace,
            &bg,
            "{.spec.database.template.spec.deployment.spec.password}",
        )
        .await?;
        if password.is_empty() {
            return Err(anyhow!("could not read DB password from BattleGroup {bg}"));
        }
        Ok(PgCredentials {
            user,
            password,
            database,
        })
    }

    async fn resolve_endpoint(&self, namespace: &str) -> Result<PgEndpoint> {
        if let Some(host) = &self.config.host_override {
            if !host.is_empty() {
                return Ok(parse_host(host));
            }
        }

        // Probe 127.0.0.1:15432 (the host port the desktop's database tunnel targets).
        if probe_loopback(DEFAULT_HOST_PORT).await {
            return Ok(PgEndpoint {
                host: "127.0.0.1".to_string(),
                port: DEFAULT_HOST_PORT,
            });
        }
        tracing::info!(
            "loopback 127.0.0.1:{} unavailable; falling back to k3s ClusterIP",
            DEFAULT_HOST_PORT
        );

        // Fall back to k3s ClusterIP for the database service.
        let cluster_ip = lookup_db_cluster_ip(&self.kubectl, namespace).await?;
        Ok(PgEndpoint {
            host: cluster_ip,
            port: DEFAULT_CLUSTER_PORT,
        })
    }
}

impl ClientState {
    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn client_mut(&mut self) -> &mut Client {
        &mut self.client
    }
}

fn parse_host(raw: &str) -> PgEndpoint {
    if let Some((host, port)) = raw.rsplit_once(':') {
        if let Ok(p) = port.parse::<u16>() {
            return PgEndpoint {
                host: host.to_string(),
                port: p,
            };
        }
    }
    PgEndpoint {
        host: raw.to_string(),
        port: DEFAULT_HOST_PORT,
    }
}

async fn probe_loopback(port: u16) -> bool {
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
        Ok(Ok(_)) => true,
        Ok(Err(_)) | Err(_) => false,
    }
}

async fn lookup_db_cluster_ip(kubectl: &KubectlClient, namespace: &str) -> Result<String> {
    let result = kubectl
        .run(&[
            "get",
            "svc",
            "-n",
            namespace,
            "-o",
            "custom-columns=NAME:.metadata.name,IP:.spec.clusterIP",
            "--no-headers",
        ])
        .await?;
    result.require_ok(&format!("kubectl get svc -n {namespace}"))?;

    let mut candidate: Option<String> = None;
    for line in result.stdout.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let name = parts.next().unwrap_or("");
        let ip = parts.next().unwrap_or("");
        if ip == "None" || ip.is_empty() {
            continue;
        }
        if name.contains("db")
            && (name.contains("postgres") || name.contains("dbdepl") || name.contains("database"))
        {
            candidate = Some(ip.to_string());
            break;
        }
    }
    candidate.ok_or_else(|| {
        anyhow!("could not locate database service ClusterIP in namespace {namespace}")
    })
}

/// Escape a value for use in a libpq key=value connection string. Wraps in
/// single quotes and escapes embedded backslashes/quotes.
fn shell_escape_keyvalue(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{escaped}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_host_picks_up_explicit_port() {
        let e = parse_host("10.0.0.5:25432");
        assert_eq!(e.host, "10.0.0.5");
        assert_eq!(e.port, 25432);
    }

    #[test]
    fn parse_host_defaults_when_no_port() {
        let e = parse_host("postgres.example");
        assert_eq!(e.host, "postgres.example");
        assert_eq!(e.port, DEFAULT_HOST_PORT);
    }

    #[test]
    fn shell_escape_wraps_and_escapes() {
        assert_eq!(shell_escape_keyvalue("simple"), "'simple'");
        assert_eq!(shell_escape_keyvalue("with'quote"), "'with\\'quote'");
        assert_eq!(shell_escape_keyvalue("back\\slash"), "'back\\\\slash'");
    }
}
